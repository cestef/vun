//! The timings of a run, as a table: per activity its runs, own and total time, and its longest
//! run.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use vun_data::{Object, Value};
use vun_term::{Column, Record, Status, Style, Table, Text, Verb};

use super::{Activity, Kind, TABLE, Timings};

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
    /// The total time of its longest run.
    pub longest: Duration,
}

impl Timings {
    /// Every activity timed so far, the most own time first; `None` when timing is off.
    #[must_use]
    pub fn report() -> Option<Report> {
        let started = Self::started()?;
        let nanos = |n: &AtomicU64| Duration::from_nanos(n.load(Ordering::Relaxed));
        let mut rows: Vec<Row> = TABLE
            .pin()
            .iter()
            .map(|(&activity, sums)| Row {
                activity,
                runs: sums.runs.load(Ordering::Relaxed),
                own: nanos(&sums.own),
                total: nanos(&sums.total),
                longest: nanos(&sums.longest),
            })
            .collect();
        rows.sort_by(|a, b| b.own.cmp(&a.own).then(a.activity.cmp(&b.activity)));
        Some(Report { wall: started.elapsed(), rows })
    }
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
            Self::Read => "read",
            Self::Component => "component",
        })
    }
}

/// The columns of the report.
const COLUMNS: [Column; 5] = [
    Column::right("own"),
    Column::right("total"),
    Column::right("longest"),
    Column::right("runs"),
    Column::left("activity"),
];

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
            let Row { activity, runs, own, total, longest } = row;
            let name =
                Text::of(Style::Dim, format!("{} ", activity.kind)).styled(Style::Literal, activity.name);
            [
                Text::from(Millis(*own).to_string()),
                Text::from(Millis(*total).to_string()),
                Text::from(Millis(*longest).to_string()),
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
                    .with("total_us", micros(r.total))
                    .with("longest_us", micros(r.longest));
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
