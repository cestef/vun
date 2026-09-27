//! Where the time goes: each activity (a driver stage, a query) timed every time it runs, summed
//! per activity over every thread. An activity's own time excludes the activities it ran inside
//! it, so own times add up to the work done; its total time includes them.
//!
//! Off by default, when timing an activity costs one relaxed load. `VUN_TIMINGS=1` or
//! `--timings` turns it on, and the report is shown at exit.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, OnceLock};
use std::time::{Duration, Instant};

use vun_json::{Object, Value};
use vun_par::Map;
use vun_term::{Column, Record, Status, Style, Table, Text, Verb};

/// What is timed: its kind, and its name among those of its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Activity {
    pub kind: Kind,
    pub name: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    /// A step of the driver, as reading packages or generating code.
    Stage,
    /// A query computed.
    Query,
}

/// The switch and the table of every activity timed.
pub struct Timings;

/// The sums of one activity, in nanoseconds.
#[derive(Default)]
struct Sums {
    runs: AtomicU64,
    own: AtomicU64,
    total: AtomicU64,
}

/// An activity running on this thread: when it started, and the time of those it ran inside it.
struct Open {
    started: Instant,
    inner: Duration,
}

static ON: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<Instant> = OnceLock::new();
static TABLE: LazyLock<Map<Activity, Arc<Sums>>> = LazyLock::new(Map::default);

thread_local! {
    /// The activities running on this thread, innermost last.
    static OPEN: RefCell<Vec<Open>> = const { RefCell::new(Vec::new()) };
    /// This thread's handles to the sums, so that timing takes no shared lookup.
    static SUMS: RefCell<HashMap<Activity, Arc<Sums>>> = RefCell::new(HashMap::new());
}

/// An activity being timed; its time is added when it is dropped, on the thread that started it,
/// whose open activities nest.
#[must_use = "the activity is timed until the timer is dropped"]
pub struct Timer(Option<Arc<Sums>>, PhantomData<*const ()>);

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

    /// Every activity timed so far, the most own time first; `None` when timing is off.
    #[must_use]
    pub fn report() -> Option<Report> {
        let started = *STARTED.get()?;
        let nanos = |n: &AtomicU64| Duration::from_nanos(n.load(Ordering::Relaxed));
        let mut rows: Vec<Row> = TABLE
            .pin()
            .iter()
            .map(|(&activity, sums)| Row {
                activity,
                runs: sums.runs.load(Ordering::Relaxed),
                own: nanos(&sums.own),
                total: nanos(&sums.total),
            })
            .collect();
        rows.sort_by(|a, b| b.own.cmp(&a.own).then(a.activity.cmp(&b.activity)));
        Some(Report { wall: started.elapsed(), rows })
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

    /// Times the activity until the timer returned is dropped.
    pub fn start(self) -> Timer {
        if !Timings::enabled() {
            return Timer(None, PhantomData);
        }
        let sums = SUMS.with_borrow_mut(|sums| {
            Arc::clone(
                sums.entry(self)
                    .or_insert_with(|| Arc::clone(TABLE.pin().get_or_insert_with(self, Arc::default))),
            )
        });
        OPEN.with_borrow_mut(|open| open.push(Open { started: Instant::now(), inner: Duration::ZERO }));
        Timer(Some(sums), PhantomData)
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let Some(sums) = self.0.take() else { return };
        let (total, inner) = OPEN.with_borrow_mut(|open| {
            let this = open.pop().expect("a timer closes the activity it opened");
            let total = this.started.elapsed();
            if let Some(outer) = open.last_mut() {
                outer.inner += total;
            }
            (total, this.inner)
        });
        let nanos = |d: Duration| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
        sums.runs.fetch_add(1, Ordering::Relaxed);
        sums.own.fetch_add(nanos(total.saturating_sub(inner)), Ordering::Relaxed);
        sums.total.fetch_add(nanos(total), Ordering::Relaxed);
    }
}

/// The timings of a run, shown as a table.
#[derive(Clone, Debug)]
pub struct Report {
    /// Since timing started.
    pub wall: Duration,
    pub rows: Vec<Row>,
}

#[derive(Clone, Copy, Debug)]
pub struct Row {
    pub activity: Activity,
    pub runs: u64,
    /// Summed over every thread, so it can exceed the wall time.
    pub own: Duration,
    /// Including activities run inside it, and nested runs of itself counted again.
    pub total: Duration,
}

/// Milliseconds with two decimals.
struct Millis(Duration);

impl fmt::Display for Millis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.2} ms", self.0.as_secs_f64() * 1e3)
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stage => "stage",
            Self::Query => "query",
        })
    }
}

/// The columns of the report.
const COLUMNS: [Column; 4] =
    [Column::right("own"), Column::right("total"), Column::right("runs"), Column::left("activity")];

impl Report {
    /// The work done: own times summed over every activity.
    fn work(&self) -> Duration {
        self.rows.iter().map(|r| r.own).sum()
    }
}

impl Record for Report {
    fn kind(&self) -> &'static str {
        "timings"
    }

    /// The wall time and the work, then a row per activity, most own time first.
    fn text(&self) -> Text {
        let title = Text::of(Style::Strong, Millis(self.wall).to_string()).plain(" wall, ");
        let title =
            title.styled(Style::Strong, Millis(self.work()).to_string()).plain(" of work over every thread");
        let rows = self.rows.iter().map(|row| {
            let Row { activity, runs, own, total } = row;
            let name =
                Text::of(Style::Dim, format!("{} ", activity.kind)).styled(Style::Literal, activity.name);
            [
                Text::from(Millis(*own).to_string()),
                Text::from(Millis(*total).to_string()),
                Text::from(runs.to_string()),
                name,
            ]
        });
        title.newline().append(Table::new(&COLUMNS).rows(rows).text())
    }

    fn fields(&self) -> Object<'static> {
        let micros = |d: Duration| u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        let rows: Vec<Value<'static>> = self
            .rows
            .iter()
            .map(|r| {
                let activity =
                    Object::new().with("kind", r.activity.kind.to_string()).with("name", r.activity.name);
                let row = activity
                    .with("runs", r.runs)
                    .with("own_us", micros(r.own))
                    .with("total_us", micros(r.total));
                Value::Object(row)
            })
            .collect();
        Object::new()
            .with("wall_us", micros(self.wall))
            .with("work_us", micros(self.work()))
            .with("activities", rows)
    }
}

impl Status for Report {
    fn verb(&self) -> Verb {
        Verb::good("Timings")
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.text().fmt(f)
    }
}
