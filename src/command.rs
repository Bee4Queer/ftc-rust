//! A command system, similar to `FTCLib`'s.

use std::{
    any::type_name,
    collections::{HashMap, VecDeque},
    convert::Infallible,
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, LazyLock, atomic::AtomicUsize},
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
        command_i: AtomicUsize::new(0),
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

/// An ID identifying a [`StoredCommand`].
type CommandId = usize;
/// The data stored that represents a command.
type StoredCommand = (Box<dyn Command>, CommandState);

/// The command scheduler.
pub struct CommandScheduler {
    /// Current length of the queue for the current round.
    queue_len: Arc<AtomicUsize>,
    /// Condvar for the queue being empty.
    empty: Arc<Condvar>,
    /// Mutex used with the empty condvar.
    empty_mutex: Arc<Mutex<bool>>,
    /// Counter used to assign command IDs.
    command_i: AtomicUsize,
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
        self.commands
            .lock()
            .insert(id, (Box::new(command), CommandState::Initializing));
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
                .name("OpModeCommandScheduler".to_string())
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
                            for (cmd, state) in commands_locked.values_mut() {
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
                                                if cmd.try_run(&ctx) {
                                                    cmd.execute(&ctx);
                                                }
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
            .1
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
            .1 = CommandState::Finished;
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
    fn execute(&mut self, ctx: &FtcContext);
    /// Whether to attempt to run this command. If not overridden, always
    /// returns true.
    ///
    /// Only called during the execute phase, and called before actually running it.
    fn try_run(&self, ctx: &FtcContext) -> bool {
        true
    }
    /// Return whether this command has finished or not. If not overridden,
    /// always returns false (meaning it runs forever).
    fn is_finished(&self, ctx: &FtcContext) -> bool {
        false
    }
    /// Ran after [`Command::is_finished`] returns true.
    fn end(&mut self, ctx: &FtcContext) {}
    /// Schedule this command. For no-op commands like () or Infalliable, does
    /// nothing.
    fn schedule(self) -> CommandHandle
    where
        Self: Sized,
    {
        SCHEDULER.write().execute(self)
    }
}

impl Command for () {
    fn name(&self) -> String {
        "unit command".to_string()
    }
    fn execute(&mut self, _: &FtcContext) {}
    fn is_finished(&self, _: &FtcContext) -> bool {
        true
    }
    fn try_run(&self, _: &FtcContext) -> bool {
        false
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
    fn is_finished(&self, _: &FtcContext) -> bool {
        match *self {}
    }
    fn try_run(&self, _: &FtcContext) -> bool {
        match *self {}
    }
    fn end(&mut self, _: &FtcContext) {
        match *self {}
    }
    fn schedule(self) -> CommandHandle
    where
        Self: Sized,
    {
        match self {} // no point in attempting to schedule this, as this point is unreachable
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
    fn try_run(&self, ctx: &FtcContext) -> bool {
        if let Some(cmd) = self.front() {
            cmd.try_run(ctx)
        } else {
            false
        }
    }
    fn is_finished(&self, _: &FtcContext) -> bool {
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
    fn try_run(&self, ctx: &FtcContext) -> bool {
        if let Some(cmd) = self.first() {
            cmd.try_run(ctx)
        } else {
            false
        }
    }
    fn is_finished(&self, _: &FtcContext) -> bool {
        self.is_empty()
    }
}
