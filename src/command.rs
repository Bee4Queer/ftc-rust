//! A command system, similar to `FTCLib`'s.

use std::{
    any::type_name,
    collections::{HashMap, VecDeque},
    convert::Infallible,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, AtomicUsize},
    },
    thread::JoinHandle,
    time::Duration,
};

use log::error;
use parking_lot::{Condvar, Mutex, RwLock, RwLockReadGuard};

use crate::{FtcContext, PanicText, take_panic_text};

/// The scheduler singleton.
pub(crate) static SCHEDULER: LazyLock<RwLock<CommandScheduler>> = LazyLock::new(|| {
    RwLock::new(CommandScheduler {
        empty: Arc::new(Condvar::new()),
        empty_mutex: Arc::new(Mutex::new(false)),
        queue_len: Arc::new(AtomicUsize::new(0)),
        command_i: AtomicU64::new(0),
        commands: Arc::new(Mutex::new(HashMap::with_capacity(16))),
        runner_thread: None,
    })
});

/// Get the scheduler. Should generally not be used as most methods are
/// otherwise available on other types. Shouldn't be used to schedule commands,
/// use the method [`schedule`](Command::schedule) available on all
/// [`Command`]s.
pub fn get_scheduler<'a>() -> RwLockReadGuard<'a, CommandScheduler> {
    SCHEDULER.read()
}

/// The current state of a command.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum CommandState {
    /// The command has not been initialized yet.
    #[default]
    Initializing,
    /// Continualy execute.
    Executing,
    /// Command has finished.
    Finished,
    /// Command panicked and will not be executed again.
    Panicked(PanicText),
}

impl CommandState {
    /// Whether this state represents a command that is no longer running (`Finished` or
    /// `Panicked`).
    pub fn ended(&self) -> bool {
        matches!(self, Self::Finished | Self::Panicked(_))
    }
    /// The text for a panic from the associated command, if one exists.
    pub fn panic_text(self) -> Option<PanicText> {
        match self {
            Self::Panicked(text) => Some(text),
            _ => None,
        }
    }
}

/// An ID identifying a [`StoredCommand`].
type CommandId = u64;

/// The data stored that represents a command.
struct StoredCommand {
    /// The actual command.
    cmd: Box<dyn Command>,
    /// The current state of the command.
    state: CommandState,
    /// Commands to schedule after the command finishes.
    schedule_after: Vec<Box<dyn Command>>,
}

/// The command scheduler.
pub struct CommandScheduler {
    /// Current length of the queue for the current round.
    queue_len: Arc<AtomicUsize>,
    /// Condvar for the queue being empty.
    empty: Arc<Condvar>,
    /// Mutex used with the empty condvar.
    empty_mutex: Arc<Mutex<bool>>,
    /// Counter used to assign command IDs.
    command_i: AtomicU64,
    /// Stored commands
    commands: Arc<Mutex<HashMap<CommandId, StoredCommand>>>,
    /// The runner thread.
    runner_thread: Option<(JoinHandle<()>, Arc<Condvar>)>,
}

impl Debug for CommandScheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandScheduler")
            .field("queue_len", &self.queue_len())
            .finish()
    }
}

impl CommandScheduler {
    /// Return the length of the command queue.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue_len.load(std::sync::atomic::Ordering::Acquire)
    }
    /// Execute this command.
    pub fn execute(&self, command: impl Command) -> CommandHandle {
        let id = self
            .command_i
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.commands.lock().insert(
            id,
            StoredCommand {
                cmd: Box::new(command),
                state: CommandState::Initializing,
                schedule_after: Vec::new(),
            },
        );
        CommandHandle { id }
    }
    /// Waits until the queue is clear.
    pub fn wait_until_queue_clear(&self) {
        if self.queue_len() == 0 {
            return;
        }
        self.empty.wait(&mut self.empty_mutex.lock());
    }
    /// Stop the scheduler. Will kill any active commands.
    pub(crate) fn stop(&mut self) {
        if let Some((join_handle, kill)) = self.runner_thread.take() {
            kill.notify_all();

            let _ = join_handle.join();
            self.queue_len
                .store(0usize, std::sync::atomic::Ordering::Release);
            *self.empty_mutex.lock() = true;
            self.empty.notify_all();
            *self.empty_mutex.lock() = false;
        }
    }
    /// Run this scheduler.
    pub(crate) fn run(&mut self, ctx: FtcContext) {
        let commands = self.commands.clone();
        let empty = self.empty.clone();
        let empty_mutex = self.empty_mutex.clone();
        let queue_len = self.queue_len.clone();
        let kill = Arc::new(Condvar::new());
        let (kill_runner, kill_mutex_runner) = (kill.clone(), Mutex::new(false));

        self.runner_thread = Some((
            std::thread::Builder::new()
                .name("op mode command scheduler".to_string())
                .spawn(move || {
                    ctx.init_thread_silent();
                    loop {
                        if !kill_runner
                            .wait_for(&mut kill_mutex_runner.lock(), Duration::from_millis(10))
                            .timed_out()
                        {
                            return;
                        }
                        let mut commands_locked = commands.lock();

                        queue_len
                            .store(commands_locked.len(), std::sync::atomic::Ordering::Release);

                        std::thread::scope(|s| {
                            for StoredCommand {
                                cmd,
                                state,
                                schedule_after,
                            } in commands_locked.values_mut()
                            {
                                let ctx = ctx.clone();
                                s.spawn(move || {
                                    ctx.init_thread();
                                    let res = catch_unwind(AssertUnwindSafe(|| {
                                        match state {
                                            CommandState::Finished => {}
                                            CommandState::Panicked(_) => {}
                                            CommandState::Initializing => {
                                                cmd.init(&ctx);
                                                *state = CommandState::Executing;
                                            }
                                            CommandState::Executing => {
                                                cmd.execute(&ctx);
                                            }
                                        }
                                        if *state != CommandState::Finished
                                            && !matches!(*state, CommandState::Panicked(_))
                                            && cmd.is_finished(&ctx)
                                        {
                                            *state = CommandState::Finished;
                                            cmd.end(&ctx);
                                        }
                                    }));
                                    if state.ended() {
                                        for command in schedule_after.drain(..) {
                                            command.schedule();
                                        }
                                    }
                                    match res {
                                        Ok(()) => {}
                                        Err(_) => {
                                            let s = take_panic_text();
                                            *state = CommandState::Panicked(s.clone());
                                            error!(
                                                "command panicked in opmode {} (halting command): \
                                                 {}\n{}",
                                                ctx.id(),
                                                s.with_location,
                                                s.backtrace
                                            );
                                        }
                                    }
                                });
                            }
                        });

                        *empty_mutex.lock() = true;
                        empty.notify_all();
                        *empty_mutex.lock() = false;
                        std::thread::yield_now();
                    }
                })
                .unwrap(),
            kill,
        ));
    }
}

/// A reference to a running command.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct CommandHandle {
    /// The internal ID.
    id: CommandId,
}

impl Debug for CommandHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(opaque Command handle)")
    }
}

impl CommandHandle {
    /// The current state of this command.
    #[must_use]
    pub fn state(&self) -> CommandState {
        get_scheduler()
            .commands
            .lock()
            .get(&self.id)
            .unwrap()
            .state
            .clone()
    }
    /// Stop this command. This will not halt it if it is currently executing and locked up, but it
    /// will not run it on the next scheduler cycle.
    pub fn stop(&self) {
        SCHEDULER
            .write()
            .commands
            .lock()
            .get_mut(&self.id)
            .unwrap()
            .state = CommandState::Finished;
    }
    /// Run the provided command after this command finishes.
    pub fn run_after(&self, cmd: impl Command) {
        SCHEDULER
            .write()
            .commands
            .lock()
            .get_mut(&self.id)
            .unwrap()
            .schedule_after
            .push(Box::new(cmd));
    }
}

struct Sequence<A: Command, B: Command> {
    a: A,
    a_done: bool,
    b: B,
}

impl<A: Command, B: Command> Command for Sequence<A, B> {
    fn init(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.init(ctx);
        } else {
            self.b.init(ctx);
        }
    }
    fn execute(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.execute(ctx);
        } else {
            self.b.execute(ctx);
        }
    }
    fn is_finished(&mut self, ctx: &FtcContext) -> bool {
        if !self.a_done {
            self.a_done = self.a.is_finished(ctx);
            false
        } else {
            self.b.is_finished(ctx)
        }
    }
    fn end(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.end(ctx);
        } else {
            self.b.end(ctx);
        }
    }
    fn name(&self) -> String {
        format!("Sequence<{}, {}>", self.a.name(), self.b.name())
    }
}

struct Parallel<A: Command, B: Command> {
    a: A,
    a_done: bool,
    b: B,
    b_done: bool,
}

impl<A: Command, B: Command> Command for Parallel<A, B> {
    fn init(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.init(ctx);
        }
        if !self.b_done {
            self.b.init(ctx);
        }
    }
    fn execute(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.execute(ctx);
        }
        if !self.b_done {
            self.b.execute(ctx);
        }
    }
    fn is_finished(&mut self, ctx: &FtcContext) -> bool {
        if !self.a_done {
            self.a_done = self.a.is_finished(ctx);
        }
        if !self.b_done {
            self.b_done = self.b.is_finished(ctx);
        }
        self.a_done && self.b_done
    }
    fn end(&mut self, ctx: &FtcContext) {
        if !self.a_done {
            self.a.end(ctx);
        }
        if !self.b_done {
            self.b.end(ctx);
        }
    }
    fn name(&self) -> String {
        format!("Parallel<{}, {}>", self.a.name(), self.b.name())
    }
}

/// A command. Forms the foundation of the command system.
#[allow(unused_variables)]
pub trait Command: Send + Sync + 'static {
    /// A debug-friendly name of this command.
    fn name(&self) -> String {
        type_name::<Self>().to_string()
    }
    /// Initialize this command.
    fn init(&mut self, ctx: &FtcContext) {}
    /// Execute this command. Called in a loop and should not block for too long
    /// for risk of holding up the command queue.
    fn execute(&mut self, ctx: &FtcContext) {}
    /// Return whether this command has finished or not. If not overridden,
    /// always returns false (meaning it runs forever).
    fn is_finished(&mut self, ctx: &FtcContext) -> bool {
        false
    }
    /// Ran after [`Command::is_finished`] returns true.
    fn end(&mut self, ctx: &FtcContext) {}
    /// After this command finishes, run this command.
    fn sequence(self, next: impl Command) -> impl Command
    where
        Self: Sized,
    {
        Sequence {
            a: self,
            a_done: false,
            b: next,
        }
    }
    /// Run these two commands at the same time
    fn parallel(self, next: impl Command) -> impl Command
    where
        Self: Sized,
    {
        Parallel {
            a: self,
            a_done: false,
            b: next,
            b_done: false,
        }
    }

    /// Schedule this command.
    fn schedule(self) -> CommandHandle
    where
        Self: Sized,
    {
        SCHEDULER.write().execute(self)
    }
}

/// A type that can be converted to a command.
pub trait IntoCommand {
    /// The type of the command it is converted into.
    type Cmd: Command + Sized;
    /// Convert this into a command.
    fn into_command(self) -> Self::Cmd;

    /// Schedule this command.
    fn schedule(self) -> CommandHandle
    where
        Self: Sized,
    {
        self.into_command().schedule()
    }
}

impl<C: Command + ?Sized> Command for Box<C> {
    fn init(&mut self, ctx: &FtcContext) {
        (**self).init(ctx);
    }
    fn execute(&mut self, ctx: &FtcContext) {
        (**self).execute(ctx);
    }
    fn end(&mut self, ctx: &FtcContext) {
        (**self).end(ctx);
    }
    fn is_finished(&mut self, ctx: &FtcContext) -> bool {
        (**self).is_finished(ctx)
    }
    fn name(&self) -> String {
        (**self).name()
    }
}

impl Command for () {
    fn name(&self) -> String {
        "unit command".to_string()
    }
    fn execute(&mut self, _: &FtcContext) {}
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        true
    }
}

impl Command for Infallible {
    fn name(&self) -> String {
        match *self {}
    }
    fn init(&mut self, _: &FtcContext) {
        match *self {}
    }
    fn execute(&mut self, _: &FtcContext) {
        match *self {}
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        match *self {}
    }
    fn end(&mut self, _: &FtcContext) {
        match *self {}
    }
}

impl<T: Command> Command for VecDeque<T> {
    fn name(&self) -> String {
        format!(
            "VecDeque<{}>",
            self.front()
                .map(Command::name)
                .unwrap_or_else(|| "(empty)".to_string())
        )
    }
    fn init(&mut self, ctx: &FtcContext) {
        if let Some(cmd) = self.front_mut() {
            cmd.init(ctx);
        }
    }
    fn execute(&mut self, ctx: &FtcContext) {
        if let Some(cmd) = self.front_mut() {
            cmd.execute(ctx);
            if cmd.is_finished(ctx) {
                cmd.end(ctx);
                self.pop_front();
                if let Some(cmd) = self.front_mut() {
                    cmd.init(ctx);
                }
            }
        }
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        self.is_empty()
    }
}

impl<T: Command> Command for Vec<T> {
    fn name(&self) -> String {
        format!(
            "Vec<{}>",
            self.first()
                .map(Command::name)
                .unwrap_or("(unknown type)".to_string())
        )
    }
    fn init(&mut self, ctx: &FtcContext) {
        if let Some(cmd) = self.first_mut() {
            cmd.init(ctx);
        }
    }
    fn execute(&mut self, ctx: &FtcContext) {
        if let Some(cmd) = self.first_mut() {
            cmd.execute(ctx);
            if cmd.is_finished(ctx) {
                cmd.end(ctx);
                self.remove(0);
                if let Some(cmd) = self.first_mut() {
                    cmd.init(ctx);
                }
            }
        }
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        self.is_empty()
    }
}

impl<T: Command, V: IntoIterator<Item = T>> IntoCommand for V {
    type Cmd = Vec<T>;
    fn into_command(self) -> Self::Cmd {
        self.into_iter().collect()
    }
}

