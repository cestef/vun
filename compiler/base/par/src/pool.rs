//! The threads every `par_map` of the process shares, started once, so that a call costs a queue
//! push and a wake-up rather than a thread per worker.
//!
//! A call lends its work, a closure borrowing the caller's stack, to workers idle at that moment,
//! runs its own share, and waits until every worker lent it is done with it. Lending only to idle
//! workers keeps what the caller waits on running, never queued behind other work: a worker never
//! waits for a call, so a caller holding a query that a busy worker needs cannot wait on it.
//!
//! This is the one module of the compiler that uses `unsafe` (the workspace denies it): the
//! closure's lifetime is erased to send it to a thread that outlives the call. The invariant that
//! makes it sound is local to [`Pool::run`]: the frame owning the closure does not end, returning
//! or unwinding, before every worker it was lent to has reported to the call's latch, and a
//! worker reports only once it will never touch the closure again.

#![allow(unsafe_code, reason = "lending borrowed work to threads that outlive the call")]

use std::any::Any;
use std::collections::VecDeque;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;

use crate::{STACK, Working};

/// A panic's payload, carried from the thread it happened on to the caller.
type Panic = Box<dyn Any + Send>;

/// The shared threads, and the work lent to them.
pub(crate) struct Pool {
    state: Mutex<State>,
    /// Signalled when work is lent.
    lent: Condvar,
}

struct State {
    /// Work lent and not yet taken: never more than the workers idle, so each is taken at once.
    queue: VecDeque<Task>,
    /// Workers not running anything and not reserved for work in the queue.
    idle: usize,
}

/// Work lent to one worker: the call's closure and where to report when done with it.
struct Task {
    work: Work,
    latch: Arc<Latch>,
}

/// A call's closure, its lifetime erased so that it can be sent to a worker. Only
/// [`Pool::run`] makes one, and only a worker calls it, before reporting to the call's latch.
struct Work(*const (dyn Fn() + Sync + 'static));

// SAFETY: the closure is `Sync`, so calling it from another thread through a shared reference is
// sound; its lifetime is upheld by `Pool::run` (see the module).
unsafe impl Send for Work {}

/// How many workers lent a call's work are not done with it, and the first panic among them.
struct Latch {
    state: Mutex<Pending>,
    done: Condvar,
}

struct Pending {
    workers: usize,
    panic: Option<Panic>,
}

impl Pool {
    /// The process's pool, its workers started on first use: one fewer than the threads the
    /// machine runs at once, the caller being the last.
    pub(crate) fn global() -> &'static Pool {
        static POOL: OnceLock<Pool> = OnceLock::new();
        let mut started = false;
        let pool = POOL.get_or_init(|| {
            started = true;
            Pool { state: Mutex::new(State { queue: VecDeque::new(), idle: 0 }), lent: Condvar::new() }
        });
        if started {
            pool.start(crate::threads().saturating_sub(1));
        }
        pool
    }

    /// Starts up to `workers` threads; each that starts is idle.
    fn start(&'static self, workers: usize) {
        for _ in 0..workers {
            let spawned = thread::Builder::new().stack_size(STACK).spawn(move || self.work());
            if spawned.is_err() {
                break;
            }
            lock(&self.state).idle += 1;
        }
    }

    /// Runs `work` on the caller and on up to `helpers` idle workers at once, returning once each
    /// is done with it, with how many workers joined. A panic in the caller's run goes on once the
    /// workers are done; a panic in a worker's goes on in the caller once the caller's own run
    /// ends.
    pub(crate) fn run(&self, helpers: usize, work: &(dyn Fn() + Sync)) -> usize {
        // SAFETY: erases the lifetime of `work` alone, the pointee and its metadata unchanged. The
        // pointer goes only into the tasks queued below, each reports to `latch` once its worker
        // is done with it, and `lent` waits for every report before this frame ends, however it
        // ends: `work` outlives every use.
        #[expect(clippy::transmute_ptr_to_ptr, reason = "a cast cannot lengthen a trait object's lifetime")]
        let erased = unsafe {
            std::mem::transmute::<*const (dyn Fn() + Sync + '_), *const (dyn Fn() + Sync + 'static)>(work)
        };
        // The guard comes first: from here on, this frame ends only once every worker lent the
        // work is done with it, however it ends.
        let latch =
            Arc::new(Latch { state: Mutex::new(Pending { workers: 0, panic: None }), done: Condvar::new() });
        let lent = Lent(Arc::clone(&latch));
        let workers = {
            let mut state = lock(&self.state);
            let workers = helpers.min(state.idle);
            // Room first, so that lending below allocates nothing and cannot fail halfway.
            state.queue.reserve(workers);
            lock(&latch.state).workers = workers;
            state.idle -= workers;
            for _ in 0..workers {
                state.queue.push_back(Task { work: Work(erased), latch: Arc::clone(&latch) });
            }
            workers
        };
        self.lent.notify_all();
        work();
        if let Some(panic) = lent.wait() {
            panic::resume_unwind(panic);
        }
        workers
    }

    /// A worker: takes each task lent, runs it, reports, and is idle again.
    fn work(&self) {
        // What a task's work calls in turn runs on this thread alone (see `par_map`).
        let _working = Working::start();
        loop {
            let task = {
                let mut state = lock(&self.state);
                loop {
                    if let Some(task) = state.queue.pop_front() {
                        break task;
                    }
                    state = self.lent.wait(state).unwrap_or_else(PoisonError::into_inner);
                }
            };
            let Task { work, latch } = task;
            // SAFETY: the caller that lent `work` is waiting on `latch` for this report, so the
            // closure is alive; it is not touched after the report below.
            let ran = panic::catch_unwind(AssertUnwindSafe(|| unsafe { (*work.0)() }));
            latch.report(ran.err());
            lock(&self.state).idle += 1;
        }
    }
}

impl Latch {
    /// A worker is done with the work, having panicked with `panic` if it did.
    fn report(&self, panic: Option<Panic>) {
        let mut pending = lock(&self.state);
        pending.workers -= 1;
        if pending.panic.is_none() {
            pending.panic = panic;
        }
        if pending.workers == 0 {
            self.done.notify_all();
        }
    }
}

/// A call's latch, waited on however the call's frame ends.
struct Lent(Arc<Latch>);

impl Lent {
    /// Waits until every worker lent the work is done with it: the first panic among them. The
    /// guard then drops with nothing left to wait for.
    fn wait(self) -> Option<Panic> {
        self.until_done()
    }

    fn until_done(&self) -> Option<Panic> {
        let mut pending = lock(&self.0.state);
        while pending.workers > 0 {
            pending = self.0.done.wait(pending).unwrap_or_else(PoisonError::into_inner);
        }
        pending.panic.take()
    }
}

/// While the caller unwinds from a panic in its own run, the workers still using the work are
/// waited for: the frame holding it ends only after.
impl Drop for Lent {
    fn drop(&mut self) {
        drop(self.until_done());
    }
}

/// `mutex` locked. No lock is held while work runs, so none is poisoned by a panic in it; a lock
/// poisoned anyway holds a consistent state, each update being made whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
