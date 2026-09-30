//! The threads every `par_map` of the process shares, started once, so that a call costs a lock
//! and a wake-up rather than a thread per worker.
//!
//! A call opens its work, a closure borrowing the caller's stack, to the workers, runs its own
//! share, closes it, and waits until every worker that joined is done with it. A worker idle, at
//! the call or any time before it closes, joins the newest call open with room: work is taken by
//! whoever is free, calls made inside another's work included, never queued behind other work.
//! What a caller waits on is always running, and [`Pool::helpers`] names it, so that a thread
//! about to wait on another can see whether that one waits on it.
//!
//! This is the one module of the compiler that uses `unsafe` (the workspace denies it): the
//! closure's lifetime is erased to send it to threads that outlive the call. The invariant that
//! makes it sound is local to [`Pool::run`]: a worker joins only while the call is open, counted
//! on the call's latch under the pool's lock; the call closes under that lock before its frame
//! ends, returning or unwinding, and the frame then waits until every worker counted has
//! reported, which a worker does only once it will never touch the closure again.

#![allow(unsafe_code, reason = "lending borrowed work to threads that outlive the call")]

use std::any::Any;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::{self, ThreadId};

use vun_trace::Activity;

use crate::{STACK, Working};

/// A caller waiting, its own share done, for the workers that joined its call, as timed.
const WAIT: Activity = Activity::stage("wait for workers");

/// A panic's payload, carried from the thread it happened on to the caller.
type Panic = Box<dyn Any + Send>;

/// The shared threads, and the calls open to them.
pub struct Pool {
    state: Mutex<State>,
    /// Signalled when a call opens.
    opened: Condvar,
}

#[derive(Default)]
struct State {
    /// Calls taking workers, the newest last.
    open: Vec<Open>,
    /// The workers in the work of each thread's calls, by the calling thread.
    helping: HashMap<ThreadId, Vec<ThreadId>>,
}

/// A call open, and how many more workers it takes.
struct Open {
    call: Arc<Call>,
    room: usize,
}

/// A call's work, the thread that made it, and where the workers that joined report.
struct Call {
    work: Work,
    caller: ThreadId,
    latch: Latch,
}

/// A call's closure, its lifetime erased so that it can be sent to workers. Only [`Pool::run`]
/// makes one, and only a worker counted on the call's latch calls it, before reporting.
struct Work(*const (dyn Fn() + Sync + 'static));

// SAFETY: the closure is `Sync`, so calling it from other threads through a shared reference is
// sound; its lifetime is upheld by `Pool::run` (see the module).
unsafe impl Send for Work {}
// SAFETY: as above: workers share it only to call the `Sync` closure.
unsafe impl Sync for Work {}

/// How many workers that joined a call are not done with it, how many joined, and the first
/// panic among them.
struct Latch {
    state: Mutex<Pending>,
    done: Condvar,
}

#[derive(Default)]
struct Pending {
    workers: usize,
    joined: usize,
    panic: Option<Panic>,
}

impl Pool {
    /// The process's pool, its workers started on first use: one fewer than the threads the
    /// machine runs at once, the caller being the last.
    pub fn global() -> &'static Pool {
        static POOL: OnceLock<Pool> = OnceLock::new();
        let mut started = false;
        let pool = POOL.get_or_init(|| {
            started = true;
            Pool { state: Mutex::new(State::default()), opened: Condvar::new() }
        });
        if started {
            pool.start(crate::threads().saturating_sub(1));
        }
        pool
    }

    /// Starts up to `workers` threads.
    fn start(&'static self, workers: usize) {
        for _ in 0..workers {
            let spawned = thread::Builder::new().stack_size(STACK).spawn(move || self.work());
            if spawned.is_err() {
                break;
            }
        }
    }

    /// The workers in the work of the calls `thread` made and has not ended: those it waits on
    /// before they end.
    #[must_use]
    pub fn helpers(&self, thread: ThreadId) -> Vec<ThreadId> {
        lock(&self.state).helping.get(&thread).cloned().unwrap_or_default()
    }

    /// Runs `work` on the caller and on up to `helpers` workers, each joining while the caller
    /// runs its own share, returning once each is done with it, with how many joined. A panic in
    /// the caller's run goes on once the workers are done; a panic in a worker's goes on in the
    /// caller once the caller's own run ends.
    pub(crate) fn run(&self, helpers: usize, work: &(dyn Fn() + Sync)) -> usize {
        // SAFETY: erases the lifetime of `work` alone, the pointee and its metadata unchanged. The
        // pointer goes only into the call below, which workers join only while it is open;
        // `lent` closes it and waits for every worker that joined before this frame ends,
        // however it ends: `work` outlives every use.
        #[expect(clippy::transmute_ptr_to_ptr, reason = "a cast cannot lengthen a trait object's lifetime")]
        let erased = unsafe {
            std::mem::transmute::<*const (dyn Fn() + Sync + '_), *const (dyn Fn() + Sync + 'static)>(work)
        };
        let caller = thread::current().id();
        let call = Arc::new(Call {
            work: Work(erased),
            caller,
            latch: Latch { state: Mutex::default(), done: Condvar::new() },
        });
        // The guard comes first: from here on, this frame ends only once the call is closed and
        // every worker that joined is done with it, however it ends.
        let lent = Lent { pool: self, call: Arc::clone(&call) };
        if helpers > 0 {
            lock(&self.state).open.push(Open { call, room: helpers });
            self.opened.notify_all();
        }
        work();
        let waiting = WAIT.start();
        let (joined, panic) = lent.finish();
        drop(waiting);
        if let Some(panic) = panic {
            panic::resume_unwind(panic);
        }
        joined
    }

    /// A worker: joins each call open, runs its work, reports, and looks again.
    fn work(&self) {
        // What a call's work calls in turn knows it runs inside a call (see `par_map`).
        let _working = Working::start();
        let me = thread::current().id();
        loop {
            let call = {
                let mut state = lock(&self.state);
                loop {
                    if let Some(call) = state.join(me) {
                        break call;
                    }
                    state = self.opened.wait(state).unwrap_or_else(PoisonError::into_inner);
                }
            };
            // SAFETY: this worker is counted on the call's latch, which its caller waits on
            // before its frame ends, so the closure is alive; it is not touched after the report.
            let ran = panic::catch_unwind(AssertUnwindSafe(|| unsafe { (*call.work.0)() }));
            lock(&self.state).left(&call, me);
            call.latch.report(ran.err());
        }
    }
}

impl State {
    /// The newest call open with room, `me` counted on its latch as working on it.
    fn join(&mut self, me: ThreadId) -> Option<Arc<Call>> {
        let open = self.open.last_mut()?;
        let call = Arc::clone(&open.call);
        open.room -= 1;
        if open.room == 0 {
            self.open.pop();
        }
        let mut pending = lock(&call.latch.state);
        pending.workers += 1;
        pending.joined += 1;
        drop(pending);
        self.helping.entry(call.caller).or_default().push(me);
        Some(call)
    }

    /// `me` is done with the work of `call`.
    fn left(&mut self, call: &Call, me: ThreadId) {
        let Some(helping) = self.helping.get_mut(&call.caller) else { return };
        if let Some(at) = helping.iter().position(|&thread| thread == me) {
            helping.swap_remove(at);
        }
        if helping.is_empty() {
            self.helping.remove(&call.caller);
        }
    }

    /// Takes no more workers into `call`.
    fn close(&mut self, call: &Arc<Call>) {
        self.open.retain(|open| !Arc::ptr_eq(&open.call, call));
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

/// A call open, closed and waited on however the call's frame ends.
struct Lent<'p> {
    pool: &'p Pool,
    call: Arc<Call>,
}

impl Lent<'_> {
    /// Closes the call and waits until every worker that joined is done with it: how many
    /// joined, and the first panic among them. The guard then drops with nothing left to wait for.
    fn finish(self) -> (usize, Option<Panic>) {
        self.until_done()
    }

    fn until_done(&self) -> (usize, Option<Panic>) {
        lock(&self.pool.state).close(&self.call);
        let latch = &self.call.latch;
        let mut pending = lock(&latch.state);
        while pending.workers > 0 {
            pending = latch.done.wait(pending).unwrap_or_else(PoisonError::into_inner);
        }
        (pending.joined, pending.panic.take())
    }
}

/// While the caller unwinds from a panic in its own run, the call is closed and the workers
/// still using the work are waited for: the frame holding it ends only after.
impl Drop for Lent<'_> {
    fn drop(&mut self) {
        drop(self.until_done());
    }
}

/// `mutex` locked. No lock is held while work runs, so none is poisoned by a panic in it; a lock
/// poisoned anyway holds a consistent state, each update being made whole.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
