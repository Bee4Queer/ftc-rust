//! A command system, similar to `FTCLib`'s.

use std::{
    any::{Any, type_name},
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
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
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CommandState {
    /// The command has not been initialized yet.
    #[default]
    Uninitialized,
    /// Continually executing in a loop.
    Executing,
    /// Command has finished.
    Finished,
    /// Command panicked and will not be executed again.
    Panicked(PanicText),
    /// Some dependencies are already being used.
    DependenciesUsed(HashSet<SubsystemHandle<dyn Subsystem>>),
}

impl CommandState {
    /// Whether this state represents a command that is no longer running (`Finished` or
    /// `Panicked`).
    pub fn ended(&self) -> bool {
        matches!(
            self,
            Self::Finished | Self::Panicked(_) | Self::DependenciesUsed(_)
        )
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

type Thenable = dyn FnOnce(&FtcContext) + Send + Sync + 'static;

/// The data stored that represents a command.
struct StoredCommand {
    /// The actual command.
    cmd: Box<dyn Command>,
    /// The current state of the command.
    state: CommandState,
    /// Commands to schedule after the command finishes.
    schedule_after: Vec<Box<dyn Command>>,
    /// Stuff to call after the command finishes.
    thenables: Vec<Box<Thenable>>,
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

struct StoredSubsystem {
    subsystem: Option<Box<dyn Subsystem>>,
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
                state: CommandState::Uninitialized,
                schedule_after: Vec::new(),
                thenables: Vec::new(),
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
    #[allow(
        clippy::mutable_key_type,
        reason = "hash implementation prevents mutation in all actual scenarios"
    )]
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

                    let subsystems: Arc<
                        RwLock<HashMap<SubsystemHandle<dyn Subsystem>, StoredSubsystem>>,
                    > = Arc::new(RwLock::new(HashMap::new()));
                    let dependencies = Arc::new(RwLock::new(HashMap::new()));

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
                            for (
                                id,
                                StoredCommand {
                                    cmd,
                                    state,
                                    schedule_after,
                                    thenables,
                                },
                            ) in commands_locked.iter_mut()
                            {
                                let ctx = ctx.clone();
                                let subsystems = subsystems.clone();
                                let dependencies = dependencies.clone();

                                s.spawn(move || {
                                    ctx.init_thread();
                                    let res = catch_unwind(AssertUnwindSafe(|| {
                                        match state {
                                            CommandState::Finished
                                            | CommandState::Panicked(_)
                                            | CommandState::DependenciesUsed(_) => {}
                                            CommandState::Uninitialized => {
                                                let mut subsystems = subsystems.write();
                                                let mut dependencies = dependencies.write();

                                                let handles = cmd
                                                    .dependencies()
                                                    .into_iter()
                                                    .collect::<HashSet<_>>();
                                                let mut cmd_dependencies = HashMap::new();
                                                let mut used =
                                                    HashSet::with_capacity(handles.len());

                                                for dependency in handles {
                                                    let subsystem = &mut match subsystems
                                                        .get_mut(&dependency)
                                                    {
                                                        Some(v) => v,
                                                        None => {
                                                            subsystems.insert(
                                                                dependency,
                                                                StoredSubsystem {
                                                                    subsystem: Some(dependency
                                                                        .creator
                                                                        .expect(
                                                                            "no creator for \
                                                                             dependency",
                                                                        )(
                                                                    )),
                                                                },
                                                            );
                                                            subsystems.get_mut(&dependency).unwrap()
                                                        }
                                                    }
                                                    .subsystem;

                                                    if subsystem.is_none() {
                                                        // dependency being used already yuo binch
                                                        used.insert(dependency);
                                                        continue;
                                                    }

                                                    cmd_dependencies.insert(
                                                        dependency,
                                                        subsystem.take().unwrap(),
                                                    );
                                                }

                                                if !used.is_empty() {
                                                    *state = CommandState::DependenciesUsed(used);
                                                    return;
                                                }

                                                let cmd_dependencies = SubsystemMap {
                                                    subsystems: cmd_dependencies,
                                                };

                                                dependencies.insert(*id, cmd_dependencies);

                                                let cmd_dependencies =
                                                    dependencies.get_mut(id).unwrap();

                                                cmd.init(&ctx, cmd_dependencies);
                                                *state = CommandState::Executing;
                                            }
                                            CommandState::Executing => {
                                                let mut dependencies = dependencies.write();
                                                let cmd_dependencies =
                                                    dependencies.get_mut(id).unwrap();
                                                cmd.execute(&ctx, cmd_dependencies);
                                            }
                                        }

                                        let mut dependencies = dependencies.write();
                                        let cmd_dependencies = dependencies.get_mut(id).unwrap();

                                        if *state != CommandState::Finished
                                            && !matches!(*state, CommandState::Panicked(_))
                                            && cmd.is_finished(&ctx, cmd_dependencies)
                                        {
                                            *state = CommandState::Finished;
                                            cmd.end(&ctx, cmd_dependencies);
                                        }
                                    }));
                                    if state.ended() {
                                        for command in schedule_after.drain(..) {
                                            command.schedule();
                                        }
                                        for thenable in thenables.drain(..) {
                                            thenable(&ctx)
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
        if self.state().ended() {
            return; // don't erase the panic message if there is one
        }
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
    /// Call the provided function after the command finishes.
    pub fn then(&self, f: impl FnOnce(&FtcContext) + Send + Sync + 'static) {
        SCHEDULER
            .write()
            .commands
            .lock()
            .get_mut(&self.id)
            .unwrap()
            .thenables
            .push(Box::new(f));
    }
}

struct Sequence<A: Command, B: Command> {
    a: A,
    a_done: bool,
    b: B,
}

impl<A: Command, B: Command> Command for Sequence<A, B> {
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.init(ctx, map);
        } else {
            self.b.init(ctx, map);
        }
    }
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.execute(ctx, map);
        } else {
            self.b.execute(ctx, map);
        }
    }
    fn is_finished(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) -> bool {
        if !self.a_done {
            self.a_done = self.a.is_finished(ctx, map);
            false
        } else {
            self.b.is_finished(ctx, map)
        }
    }
    fn end(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.end(ctx, map);
        } else {
            self.b.end(ctx, map);
        }
    }
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        [self.a.dependencies(), self.b.dependencies()].concat()
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
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.init(ctx, map);
        }
        if !self.b_done {
            self.b.init(ctx, map);
        }
    }
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.execute(ctx, map);
        }
        if !self.b_done {
            self.b.execute(ctx, map);
        }
    }
    fn is_finished(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) -> bool {
        if !self.a_done {
            self.a_done = self.a.is_finished(ctx, map);
        }
        if !self.b_done {
            self.b_done = self.b.is_finished(ctx, map);
        }
        self.a_done && self.b_done
    }
    fn end(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if !self.a_done {
            self.a.end(ctx, map);
        }
        if !self.b_done {
            self.b.end(ctx, map);
        }
    }
    #[allow(clippy::mutable_key_type)]
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        let a = self.a.dependencies();
        let b = self.b.dependencies();
        let a_hs = a.iter().collect::<HashSet<_>>();
        let b_hs = b.iter().collect::<HashSet<_>>();

        if !a_hs.is_disjoint(&b_hs) {
            panic!(
                "tried to run commands {} and {} in parallel, but they share dependencies",
                self.a.name(),
                self.b.name()
            );
        }

        [a, b].concat()
    }
    fn name(&self) -> String {
        format!("Parallel<{}, {}>", self.a.name(), self.b.name())
    }
}

/// A bunch of subsystems provided for use with a command.
#[derive(Debug)]
pub struct SubsystemMap {
    subsystems: HashMap<SubsystemHandle<dyn Subsystem>, Box<dyn Subsystem>>,
}

impl SubsystemMap {
    /// Get a subsystem.
    pub fn subsystem<S: Subsystem>(&mut self, id: &'static SubsystemHandle<S>) -> &mut S {
        ((*self.subsystems.get_mut(&**id).unwrap()).as_mut() as &mut (dyn Any + Send + Sync))
            .downcast_mut()
            .unwrap()
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
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {}
    /// Execute this command. Called in a loop and should not block for too long
    /// for risk of holding up the command queue.
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {}
    /// Return whether this command has finished or not. If not overridden,
    /// always returns false (meaning it runs forever).
    fn is_finished(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) -> bool {
        false
    }
    /// Ran after [`Command::is_finished`] returns true.
    fn end(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {}
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
    /// Run these two commands at the same time.
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

    /// The dependencies of this command.
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        vec![]
    }

    /// Schedule this command.
    fn schedule(self) -> CommandHandle
    where
        Self: Sized,
    {
        SCHEDULER.write().execute(self)
    }
}

/// A subsystem that can be owned by a [`Command`].
#[allow(unused_variables)]
pub trait Subsystem: Any + Debug + Send + Sync + 'static {
    /// A debug-friendly name of this command.
    fn name(&self) -> String {
        type_name::<Self>().to_string()
    }
    /// Update this subsytem. Called once per scheduler loop.
    fn update(&mut self, ctx: &FtcContext) {}
}

/// A handle to a subsystem. If you get clippy::mutable_key_type, you can safely ignore it. The Hash
/// implementation prevents interior mutability being a concern in all real-world scenarios.
pub struct SubsystemHandle<S: Subsystem + ?Sized> {
    id: &'static AtomicU64,
    creator: Option<fn() -> Box<dyn Subsystem>>,
    _dyn: Option<&'static SubsystemHandle<dyn Subsystem>>,
    phantom: PhantomData<S>,
}

impl<S: Subsystem + ?Sized> Clone for SubsystemHandle<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: Subsystem + ?Sized> Copy for SubsystemHandle<S> {}

impl<S: Subsystem + ?Sized> Debug for SubsystemHandle<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SubsystemHandle")
            .field(&self.id.load(Ordering::Relaxed))
            .finish()
    }
}

impl<S: Subsystem + ?Sized> PartialEq for SubsystemHandle<S> {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}
impl<S: Subsystem + ?Sized> Eq for SubsystemHandle<S> {}
impl<S: Subsystem + ?Sized> Hash for SubsystemHandle<S> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

static SUBSYSTEM_ID: AtomicU64 = AtomicU64::new(1);

/// Create a new [`SubsystemHandle`].
#[macro_export]
macro_rules! new_subsystem {
    (<$ty:ty>) => {
        const {
            static ID: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
            static DYN: $crate::command::SubsystemHandle<$crate::command::Subsystem> =
                $crate::command::SubsystemHandle::new_dyn(&id, || {
                    ::std::boxed::Box(<$ty>::default())
                });
            $crate::command::SubsystemHandle::<$ty>::new(&id, Some(&DYN))
        }
    };
    (<$ty:ty> $creator:expr $(,)?) => {
        const {
            static ID: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);
            static DYN: $crate::command::SubsystemHandle<$crate::command::Subsystem> =
                $crate::command::SubsystemHandle::new_dyn(&id, || ::std::boxed::Box(($creator)()));
            $crate::command::SubsystemHandle::<$ty>::new_with(&id, $creator, Some(&DYN))
        }
    };
}

impl<S: Subsystem + Default> SubsystemHandle<S> {
    #[doc(hidden)]
    pub const fn new(
        id: &'static AtomicU64,
        mut _dyn: Option<&'static SubsystemHandle<dyn Subsystem>>,
    ) -> Self {
        Self {
            id,
            creator: Some(|| Box::new(S::default())),
            phantom: PhantomData,
            _dyn,
        }
    }
}

impl SubsystemHandle<dyn Subsystem> {
    #[doc(hidden)]
    pub const fn new_dyn(id: &'static AtomicU64, creator: fn() -> Box<dyn Subsystem>) -> Self {
        Self {
            id,
            creator: Some(creator),
            phantom: PhantomData,
            _dyn: None,
        }
    }
}

impl<S: Subsystem + ?Sized> SubsystemHandle<S> {
    #[doc(hidden)]
    pub const fn new_with(
        id: &'static AtomicU64,
        creator: fn() -> Box<dyn Subsystem>,
        _dyn: Option<&'static SubsystemHandle<dyn Subsystem>>,
    ) -> Self {
        Self {
            id,
            creator: Some(creator),
            phantom: PhantomData,
            _dyn,
        }
    }
    fn id(&self) -> u64 {
        self.id.update(Ordering::SeqCst, Ordering::SeqCst, |v| {
            if v == 0 {
                SUBSYSTEM_ID.fetch_add(1, Ordering::SeqCst)
            } else {
                v
            }
        })
    }
}

impl<S: Subsystem + Sized> Deref for SubsystemHandle<S> {
    type Target = SubsystemHandle<dyn Subsystem>;
    fn deref(&self) -> &Self::Target {
        self._dyn.as_ref().unwrap()
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
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        (**self).init(ctx, map);
    }
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        (**self).execute(ctx, map);
    }
    fn end(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        (**self).end(ctx, map);
    }
    fn is_finished(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) -> bool {
        (**self).is_finished(ctx, map)
    }
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        (**self).dependencies()
    }
    fn name(&self) -> String {
        (**self).name()
    }
}

impl Command for () {
    fn name(&self) -> String {
        "unit command".to_string()
    }
    fn is_finished(&mut self, _: &FtcContext, _: &mut SubsystemMap) -> bool {
        true
    }
}

impl Command for Infallible {
    fn name(&self) -> String {
        match *self {}
    }
    fn init(&mut self, _: &FtcContext, _: &mut SubsystemMap) {
        match *self {}
    }
    fn execute(&mut self, _: &FtcContext, _: &mut SubsystemMap) {
        match *self {}
    }
    fn is_finished(&mut self, _: &FtcContext, _: &mut SubsystemMap) -> bool {
        match *self {}
    }
    fn end(&mut self, _: &FtcContext, _: &mut SubsystemMap) {
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
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if let Some(cmd) = self.front_mut() {
            cmd.init(ctx, map);
        }
    }
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if let Some(cmd) = self.front_mut() {
            cmd.execute(ctx, map);
            if cmd.is_finished(ctx, map) {
                cmd.end(ctx, map);
                self.pop_front();
                if let Some(cmd) = self.front_mut() {
                    cmd.init(ctx, map);
                }
            }
        }
    }
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        self.iter().flat_map(|v| v.dependencies()).collect()
    }
    fn is_finished(&mut self, _: &FtcContext, _: &mut SubsystemMap) -> bool {
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
    fn init(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if let Some(cmd) = self.first_mut() {
            cmd.init(ctx, map);
        }
    }
    fn execute(&mut self, ctx: &FtcContext, map: &mut SubsystemMap) {
        if let Some(cmd) = self.first_mut() {
            cmd.execute(ctx, map);
            if cmd.is_finished(ctx, map) {
                cmd.end(ctx, map);
                self.remove(0);
                if let Some(cmd) = self.first_mut() {
                    cmd.init(ctx, map);
                }
            }
        }
    }
    fn dependencies(&self) -> Vec<SubsystemHandle<dyn Subsystem>> {
        self.iter().flat_map(|v| v.dependencies()).collect()
    }
    fn is_finished(&mut self, _: &FtcContext, _: &mut SubsystemMap) -> bool {
        self.is_empty()
    }
}

impl<T: Command, V: IntoIterator<Item = T>> IntoCommand for V {
    type Cmd = Vec<T>;
    fn into_command(self) -> Self::Cmd {
        self.into_iter().collect()
    }
}
