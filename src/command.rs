//! A command system, similar to `FTCLib`'s.

use std::{
    collections::VecDeque, convert::Infallible, fmt::Debug, sync::{Arc, Condvar, LazyLock, Mutex, RwLock, RwLockReadGuard, atomic::AtomicUsize},
};

use crate::FtcContext;

/// The scheduler singleton.
pub(crate) static SCHEDULER: LazyLock<RwLock<CommandScheduler>> = LazyLock::new(|| {
    RwLock::new(CommandScheduler {
        to_add: Arc::new(Mutex::new(Vec::with_capacity(16))),
        empty: Arc::new(Condvar::new()),
        empty_mutex: Arc::new(Mutex::new(false)),
        queue_len: Arc::new(AtomicUsize::new(0)),
    })
});

/// Get the scheduler. Should generally not be used as most methods are otherwise available on other
/// types. Cannot be used to schedule commands, use the method [`schedule`](Command::schedule)
/// available on all [`Command`]s.
pub fn get_scheduler<'a>() -> RwLockReadGuard<'a, CommandScheduler> {
    SCHEDULER.read().unwrap()
}

/// The current state of a command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum CommandState {
    /// Next step is initalizing.
    #[default]
    Initializing,
    /// Continualy execute.
    Executing,
    /// Command has finished and on the next pass should be removed.
    Finished,
}

/// The command scheduler.
pub struct CommandScheduler {
    /// The commands that will begin executing in the next round.
    to_add: Arc<Mutex<Vec<Box<dyn Command>>>>,
    queue_len: Arc<AtomicUsize>,
    empty: Arc<Condvar>,
    empty_mutex: Arc<Mutex<bool>>,
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
    pub fn execute(&self, command: impl Command) {
        self.to_add.lock().unwrap().push(Box::new(command));
    }
    /// Waits until the queue is clear.
    pub fn wait_until_queue_clear(&self) {
        if self.queue_len() == 0 {
            return;
        }
        loop {
            let guard = self.empty.wait(self.empty_mutex.lock().unwrap()).unwrap();
            if *guard {
                return;
            }
        }
    }
    /// Run this scheduler.
    pub(crate) fn run(&mut self, ctx: FtcContext) {
        let commands = Mutex::new(Vec::new());
        let to_add = self.to_add.clone();
        let empty = self.empty.clone();
        let empty_mutex = self.empty_mutex.clone();
        let queue_len = self.queue_len.clone();
        std::thread::spawn(move || {
            loop {
                let mut commands_locked = commands.lock().unwrap();
                let to_add = to_add.lock().unwrap().drain(..).collect::<Vec<_>>();
                for command in to_add {
                    commands_locked.push((command, CommandState::Initializing));
                }
                queue_len.store(commands_locked.len(), std::sync::atomic::Ordering::Release);
                while !commands_locked.is_empty() {
                    queue_len.store(commands_locked.len(), std::sync::atomic::Ordering::Release);
                    let to_remove = Arc::new(Mutex::new(Vec::with_capacity(commands_locked.len())));
                    std::thread::scope(|s| {
                        for (i, (cmd, state)) in commands_locked
                            .iter_mut()
                            .enumerate()
                        {
                            let ctx = ctx.clone();
                            let to_remove = to_remove.clone();
                            s.spawn(move || {
                                match *state {
                                    CommandState::Finished => {
                                        cmd.end(&ctx);
                                        to_remove.lock().unwrap().push(i);
                                    }
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
                                if *state != CommandState::Finished && cmd.is_finished(&ctx) {
                                    *state = CommandState::Finished;
                                }
                            });
                        }
                    });
                    let mut to_remove = to_remove.lock().unwrap().clone();
                    to_remove.sort();

                    let mut offset = 0usize;
                    for ele in to_remove {
                        commands_locked.remove(ele - offset);
                        offset += 1;
                    }
                }
                *empty_mutex.lock().unwrap() = true;
                empty.notify_all();
                *empty_mutex.lock().unwrap() = false;
                std::thread::yield_now();
            }
        });
    }
}

/// A command. Forms the foundation of the command system.
pub trait Command: Send + Sync + 'static {
    /// Initialize this command.
    #[allow(unused_variables)]
    fn init(&mut self, ctx: &FtcContext) {}
    /// Execute this command.
    fn execute(&mut self, ctx: &FtcContext);
    /// Whether to attempt to run this command. If not overridden, always returns true.
    ///
    /// Only called during the execute phase.
    #[allow(unused_variables)]
    fn try_run(&mut self, ctx: &FtcContext) -> bool {
        true
    }
    /// Return whether this command has finished or not. If not overridden, always returns false.
    #[allow(unused_variables)]
    fn is_finished(&mut self, ctx: &FtcContext) -> bool {
        false
    }
    /// Ran after [`Command::is_finished`] returns true.
    #[allow(unused_variables)]
    fn end(&mut self, ctx: &FtcContext) {}
    /// Schedule this command.
    fn schedule(self)
    where
        Self: Sized,
    {
        SCHEDULER.write().unwrap().execute(self);
    }
}

impl Command for () {
    fn execute(&mut self, _: &FtcContext) {}
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        true
    }
    fn try_run(&mut self, _: &FtcContext) -> bool {
        false
    }
    fn schedule(self)
    where
        Self: Sized,
    {
        // No point in scheduling a no-op command.
    }
}

impl Command for Infallible {
    fn execute(&mut self, _: &FtcContext) {
        match *self {}
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        match *self {}
    }
    fn try_run(&mut self, _: &FtcContext) -> bool {
        false
    }
    fn schedule(self)
    where
        Self: Sized,
    {
        // No point in scheduling infallible.
    }
}

impl<T: Command> Command for VecDeque<T> {
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
    fn try_run(&mut self, ctx: &FtcContext) -> bool {
        if let Some(cmd) = self.front_mut() {
            cmd.try_run(ctx)
        } else {
            false
        }
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        self.is_empty()
    }
}

impl<T: Command> Command for Vec<T> {
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
    fn try_run(&mut self, ctx: &FtcContext) -> bool {
        if let Some(cmd) = self.first_mut() {
            cmd.try_run(ctx)
        } else {
            false
        }
    }
    fn is_finished(&mut self, _: &FtcContext) -> bool {
        self.is_empty()
    }
}
