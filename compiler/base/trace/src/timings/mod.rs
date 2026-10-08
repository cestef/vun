//! Where the time goes: each activity (a driver stage, a query) timed every time it runs, summed
//! per activity over every thread. An activity's own time excludes the activities it ran inside
//! it, so own times add up to the work done; its total time includes them.
//!
//! Off by default, when timing an activity costs one relaxed load. `VUN_TIMINGS=1` or
//! `--timings` turns it on, and the report is shown at exit; `VUN_TIMELINE=<file>` or
//! `--timeline <file>` also records every run, written at exit as a timeline ([`Timeline`]).

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};

use vun_variants::Variants;

mod live;
mod relay;
mod report;
mod timeline;

pub use live::Live;
pub use relay::{Relay, Relaying};
pub use report::{Report, Row};
pub use timeline::Timeline;

/// What is timed: its kind, and its name among those of its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Activity {
    pub kind: Kind,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Variants)]
#[repr(u32)]
pub enum Kind {
    /// A step of the driver, as reading packages or generating code.
    Stage,
    /// A query computed.
    Query,
    /// A query's saved values read.
    Read,
    /// A call of a component, as the host sees it.
    Component,
}

impl Kind {
    /// The kind a library numbered (`kind as u32`), if this build has it.
    #[must_use]
    pub fn numbered(number: u32) -> Option<Self> {
        Self::ALL.get(usize::try_from(number).ok()?).copied()
    }
}

/// The switch and the table of every activity timed.
pub struct Timings;

/// The sums of one activity, in nanoseconds.
#[derive(Default)]
struct Sums {
    runs: AtomicU64,
    own: AtomicU64,
    total: AtomicU64,
    /// The longest run's total.
    longest: AtomicU64,
}

/// An activity running on this thread: when it started, and the time of those it ran inside it.
struct Open {
    started: Instant,
    inner: Duration,
}

static ON: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<Instant> = OnceLock::new();
/// Every activity's sums, which every thread reads and adds to without locks.
static TABLE: LazyLock<papaya::HashMap<Activity, Arc<Sums>, foldhash::fast::RandomState>> =
    LazyLock::new(papaya::HashMap::default);

thread_local! {
    /// The activities running on this thread, innermost last.
    static OPEN: RefCell<Vec<Open>> = const { RefCell::new(Vec::new()) };
    /// This thread's handles to the sums, so that timing takes no shared lookup.
    static SUMS: RefCell<HashMap<Activity, Arc<Sums>>> = RefCell::new(HashMap::new());
}

/// An activity being timed; its time is added when it is dropped, on the thread that started it,
/// whose open activities nest.
#[must_use = "the activity is timed until the timer is dropped"]
pub struct Timer {
    running: Running,
    /// The stage shown as running, until it ends.
    live: Option<&'static str>,
    thread: PhantomData<*const ()>,
}

/// Where a timer's time goes.
enum Running {
    Off,
    /// Into this process's table.
    Here(Activity, Arc<Sums>),
    /// To the host, which times it (see [`Relay`]).
    Relayed,
}

impl Timings {
    /// Starts timing, from now.
    pub fn enable() {
        STARTED.get_or_init(Instant::now);
        ON.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn enabled() -> bool {
        ON.load(Ordering::Relaxed)
    }

    /// When timing started.
    fn started() -> Option<Instant> {
        STARTED.get().copied()
    }
}

impl Activity {
    #[must_use]
    pub const fn stage(name: &'static str) -> Self {
        Self { kind: Kind::Stage, name }
    }

    #[must_use]
    pub const fn query(name: &'static str) -> Self {
        Self { kind: Kind::Query, name }
    }

    #[must_use]
    pub const fn read(name: &'static str) -> Self {
        Self { kind: Kind::Read, name }
    }

    #[must_use]
    pub const fn component(name: &'static str) -> Self {
        Self { kind: Kind::Component, name }
    }

    /// Times the activity until the timer returned is dropped: in the host's table where a
    /// relay takes it, else in this process's.
    pub fn start(self) -> Timer {
        let live = Live::enter(self);
        if !Timings::enabled() {
            return Timer { running: Running::Off, live, thread: PhantomData };
        }
        if Relaying::begin(self) {
            return Timer { running: Running::Relayed, live, thread: PhantomData };
        }
        let mut timer = self.start_here();
        timer.live = live;
        timer
    }

    /// Times the activity in this process's table, whatever relay there is.
    pub fn start_here(self) -> Timer {
        if !Timings::enabled() {
            return Timer { running: Running::Off, live: None, thread: PhantomData };
        }
        let sums = SUMS.with_borrow_mut(|sums| {
            Arc::clone(
                sums.entry(self)
                    .or_insert_with(|| Arc::clone(TABLE.pin().get_or_insert_with(self, Arc::default))),
            )
        });
        OPEN.with_borrow_mut(|open| open.push(Open { started: Instant::now(), inner: Duration::ZERO }));
        Timer { running: Running::Here(self, sums), live: None, thread: PhantomData }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(name) = self.live {
            Live::leave(name);
        }
        let (activity, sums) = match std::mem::replace(&mut self.running, Running::Off) {
            Running::Off => return,
            Running::Relayed => return Relaying::end(),
            Running::Here(activity, sums) => (activity, sums),
        };
        let (started, total, inner) = OPEN.with_borrow_mut(|open| {
            let this = open.pop().expect("a timer closes the activity it opened");
            let total = this.started.elapsed();
            if let Some(outer) = open.last_mut() {
                outer.inner += total;
            }
            (this.started, total, this.inner)
        });
        let nanos = |d: Duration| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        sums.runs.fetch_add(1, Ordering::Relaxed);
        sums.own.fetch_add(nanos(total.saturating_sub(inner)), Ordering::Relaxed);
        sums.total.fetch_add(nanos(total), Ordering::Relaxed);
        sums.longest.fetch_max(nanos(total), Ordering::Relaxed);
        Timeline::record(activity, started, total);
    }
}
