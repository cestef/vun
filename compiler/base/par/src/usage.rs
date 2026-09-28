//! How work spread over threads went, counted as it happens: the calls that spread and those run
//! on one thread, the helpers each asked for and those idle to join it, and how busy the threads
//! were while a call ran. Counting is always on, a few relaxed increments per call; [`Usage::now`]
//! reads it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use vun_json::Object;
use vun_term::{Column, Record, Status, Style, Table, Text, Verb};

/// The counts, summed over the process.
pub(crate) struct Counts {
    pub(crate) spread: AtomicU64,
    pub(crate) alone: AtomicU64,
    pub(crate) nested: AtomicU64,
    pub(crate) items: AtomicU64,
    pub(crate) asked: AtomicU64,
    pub(crate) lent: AtomicU64,
    /// Nanoseconds.
    pub(crate) wall: AtomicU64,
    /// Nanoseconds.
    pub(crate) busy: AtomicU64,
}

pub(crate) static COUNTS: Counts = Counts {
    spread: AtomicU64::new(0),
    alone: AtomicU64::new(0),
    nested: AtomicU64::new(0),
    items: AtomicU64::new(0),
    asked: AtomicU64::new(0),
    lent: AtomicU64::new(0),
    wall: AtomicU64::new(0),
    busy: AtomicU64::new(0),
};

impl Counts {
    pub(crate) fn add(counter: &AtomicU64, n: u64) {
        counter.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn add_time(counter: &AtomicU64, time: Duration) {
        Self::add(counter, u64::try_from(time.as_nanos()).unwrap_or(u64::MAX));
    }
}

/// How the process's work spread over threads so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Usage {
    /// The threads a call runs on at most: the pool's and the caller.
    pub threads: usize,
    /// Calls spread over threads.
    pub spread: u64,
    /// Calls run on the calling thread, having one item or one thread.
    pub alone: u64,
    /// Calls made from inside a spread call's work, run on that thread alone.
    pub nested: u64,
    /// The items of the calls spread.
    pub items: u64,
    /// The helpers the spread calls asked for, and those idle to join them.
    pub asked: u64,
    pub lent: u64,
    /// How long the spread calls took, summed, and how long the threads worked on them, summed
    /// over the threads.
    pub wall: Duration,
    pub busy: Duration,
}

impl Usage {
    /// The variable choosing how many threads a call runs on, the machine's by default.
    pub const VAR: &'static str = "VUN_THREADS";

    #[must_use]
    pub fn now() -> Self {
        let n = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        let c = &COUNTS;
        Self {
            threads: crate::threads(),
            spread: n(&c.spread),
            alone: n(&c.alone),
            nested: n(&c.nested),
            items: n(&c.items),
            asked: n(&c.asked),
            lent: n(&c.lent),
            wall: Duration::from_nanos(n(&c.wall)),
            busy: Duration::from_nanos(n(&c.busy)),
        }
    }

    /// The share of the threads' time the spread calls kept busy, in percent: the threads that
    /// joined a call and the threads it could have had counted alike.
    #[must_use]
    pub fn occupancy(&self) -> f64 {
        let threads = u32::try_from(self.threads).unwrap_or(u32::MAX);
        let offered = self.wall.as_secs_f64() * f64::from(threads);
        if offered == 0.0 { 0.0 } else { 100.0 * self.busy.as_secs_f64() / offered }
    }

    /// Each figure with its name.
    fn figures(&self) -> [(&'static str, String); 9] {
        let millis = |d: Duration| format!("{:.2} ms", d.as_secs_f64() * 1e3);
        [
            ("threads", self.threads.to_string()),
            ("calls spread", self.spread.to_string()),
            ("calls alone", self.alone.to_string()),
            ("calls nested", self.nested.to_string()),
            ("items spread", self.items.to_string()),
            ("helpers asked", self.asked.to_string()),
            ("helpers lent", self.lent.to_string()),
            ("wall, busy", format!("{}, {}", millis(self.wall), millis(self.busy))),
            ("occupancy", format!("{:.0}%", self.occupancy())),
        ]
    }
}

const COLUMNS: [Column; 2] = [Column::right("value"), Column::left("figure")];

impl Record for Usage {
    fn kind(&self) -> &'static str {
        "threads"
    }

    fn text(&self) -> Text {
        let rows = self.figures().map(|(name, value)| [Text::from(value), Text::of(Style::Literal, name)]);
        Text::from("how work spread over threads").newline().append(Table::new(&COLUMNS).rows(rows).text())
    }

    fn fields(&self) -> Object<'static> {
        let micros = |d: Duration| u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        Object::new()
            .with("threads", self.threads)
            .with("spread", self.spread)
            .with("alone", self.alone)
            .with("nested", self.nested)
            .with("items", self.items)
            .with("asked", self.asked)
            .with("lent", self.lent)
            .with("wall_us", micros(self.wall))
            .with("busy_us", micros(self.busy))
    }
}

impl Status for Usage {
    fn verb(&self) -> Verb {
        Verb::good("Threads")
    }
}
