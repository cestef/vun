//! Where each activity ran: every run on every thread, when it started and how long it took,
//! written at exit in the Trace Event Format that Perfetto (`ui.perfetto.dev`) and
//! `chrome://tracing` open. It shows what the sums cannot: which runs overlapped, what a thread
//! waited on, and what the wall time's longest chain of work was.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use vun_data::json::Json;
use vun_data::{Number, Object, Value};
use vun_term::{Record, Status, Style, Text, Verb};

use super::{Activity, Timings};

/// One run of an activity, on the thread numbered `thread`, `start` after timing started.
struct Run {
    activity: Activity,
    thread: u32,
    start: Duration,
    took: Duration,
}

/// Where the timeline is written, once asked for.
static PATH: OnceLock<PathBuf> = OnceLock::new();
static RUNS: boxcar::Vec<Run> = boxcar::Vec::new();
/// Each thread that ran an activity: its number and its name.
static THREADS: boxcar::Vec<(u32, Option<String>)> = boxcar::Vec::new();
static NEXT_THREAD: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// This thread's number, given when it first runs an activity.
    static THREAD: u32 = {
        let thread = NEXT_THREAD.fetch_add(1, Ordering::Relaxed);
        THREADS.push((thread, std::thread::current().name().map(str::to_owned)));
        thread
    };
}

/// A timeline written, as reported.
#[derive(Clone, Debug)]
pub struct Timeline {
    pub path: PathBuf,
    pub runs: usize,
}

impl Timeline {
    /// Records every run from now, to write at `path` at exit; timing starts too.
    pub fn record_to(path: &Path) {
        // The first path asked for wins.
        let _ = PATH.set(path.to_path_buf());
        Timings::enable();
    }

    /// Where the timeline is written, once asked for.
    #[must_use]
    pub fn path() -> Option<&'static Path> {
        PATH.get().map(PathBuf::as_path)
    }

    /// Records a run of `activity` that started at `started` and took `took`, where a timeline
    /// is asked for.
    pub(super) fn record(activity: Activity, started: Instant, took: Duration) {
        let (Some(_), Some(zero)) = (PATH.get(), Timings::started()) else { return };
        let thread = THREAD.with(|t| *t);
        RUNS.push(Run { activity, thread, start: started.saturating_duration_since(zero), took });
    }

    /// Writes the runs recorded so far; `None` when no timeline was asked for.
    ///
    /// # Errors
    /// Returns the error writing the file.
    pub fn write() -> Option<io::Result<Self>> {
        let path = PATH.get()?;
        let events: Vec<Value<'static>> = THREADS
            .iter()
            .map(|(_, (thread, name))| Self::thread_name(*thread, name.as_deref()))
            .chain(RUNS.iter().map(|(_, run)| Self::event(run)))
            .collect();
        let runs = RUNS.count();
        let trace = Object::new().with("traceEvents", events).with("displayTimeUnit", "ms");
        Some(std::fs::write(path, Json(&trace).to_string()).map(|()| Self { path: path.clone(), runs }))
    }

    /// A complete event: a run, its start and duration in microseconds.
    fn event(run: &Run) -> Value<'static> {
        let micros = |d: Duration| Number::from_f64(d.as_secs_f64() * 1e6).map_or(Value::Null, Value::Number);
        let event = Object::new()
            .with("name", run.activity.name)
            .with("cat", run.activity.kind.to_string())
            .with("ph", "X")
            .with("ts", micros(run.start))
            .with("dur", micros(run.took))
            .with("pid", 1)
            .with("tid", run.thread);
        Value::Object(event)
    }

    /// A metadata event naming the thread numbered `thread`, by its name where it has one.
    fn thread_name(thread: u32, name: Option<&str>) -> Value<'static> {
        let name = name.map_or_else(|| format!("thread {thread}"), str::to_owned);
        let event = Object::new()
            .with("name", "thread_name")
            .with("ph", "M")
            .with("pid", 1)
            .with("tid", thread)
            .with("args", Object::new().with("name", name));
        Value::Object(event)
    }
}

impl Record for Timeline {
    fn kind(&self) -> &'static str {
        "timeline"
    }

    fn text(&self) -> Text {
        Text::from(format!("{} runs to ", self.runs)).styled(Style::Literal, self.path.display().to_string())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("path", self.path.display().to_string()).with("runs", self.runs)
    }
}

impl Status for Timeline {
    fn verb(&self) -> Verb {
        Verb::good("Timeline")
    }
}
