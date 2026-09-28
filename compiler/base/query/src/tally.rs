//! What a run did with each query's memos, counted as it happens: memos restored from the saved
//! ones, their keys and values read, memos found to hold in order or ahead, and each computation
//! by why it ran. Counting is always on, a relaxed increment each; [`Db::tally`] reads it. On one
//! thread (`VUN_THREADS=1`) the counts of a run are the same every time; over threads, work two
//! threads both do is counted twice.

use std::sync::atomic::{AtomicU64, Ordering};

use vun_json::{Object, Value};
use vun_term::{Column, Record, Status, Style, Table, Text, Verb};

use super::db::Db;

/// Declares the events once: the enum, every event in order, and each one's column name.
macro_rules! events {
    ($($(#[$doc:meta])* $event:ident => $name:literal,)*) => {
        /// Something done with a memo.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Event {
            $($(#[$doc])* $event,)*
        }

        impl Event {
            /// Every event, in the order of the report's columns.
            pub const ALL: &[Self] = &[$(Self::$event),*];

            /// Its column in the report.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$event => $name,)*
                }
            }
        }
    };
}

events! {
    /// A memo read from the saved ones.
    Restored => "restored",
    /// A saved key read.
    KeyRead => "keys",
    /// A saved value read.
    ValueRead => "values",
    /// A memo of an earlier revision or process found to hold by verifying its reads in order.
    Held => "held",
    /// A memo found to hold by verifying ahead, over threads.
    Settled => "settled",
    /// A memo reading shared inputs alone found to hold at once, their fingerprint unchanged.
    Durable => "durable",
    /// Computed: there was no memo.
    Absent => "absent",
    /// Computed: a read changed.
    Stale => "stale",
    /// Computed: the memo holds, but was saved without its value.
    Unloaded => "unloaded",
    /// Computed: the memo read one no query of the process holds.
    Unresolved => "unresolved",
    /// Computed: verifying its reads reached it again.
    Cycle => "cycle",
    /// Computed again to check a memo reused (`VUN_QUERY_CHECK=1`).
    Checked => "checked",
    /// Computed from a cycle value, and not kept.
    Provisional => "provisional",
    /// Waited for another thread computing it.
    Waited => "waited",
}

/// A table's count of each event.
#[derive(Default)]
pub(crate) struct Counts([AtomicU64; Event::ALL.len()]);

impl Counts {
    pub(crate) fn add(&self, event: Event) {
        self.0[event as usize].fetch_add(1, Ordering::Relaxed);
    }

    fn read(&self) -> [u64; Event::ALL.len()] {
        std::array::from_fn(|at| self.0[at].load(Ordering::Relaxed))
    }
}

/// What a run did with each query's memos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tally {
    /// Each query that did something, by name, with its count of each event of [`Event::ALL`].
    pub rows: Vec<(&'static str, [u64; Event::ALL.len()])>,
}

impl Tally {
    /// The variable asking for the report at exit with `1`.
    pub const VAR: &'static str = "VUN_MEMOS";

    /// The count of `event` for the query named `query`.
    #[must_use]
    pub fn count(&self, query: &str, event: Event) -> u64 {
        self.rows.iter().find(|(name, _)| *name == query).map_or(0, |(_, counts)| counts[event as usize])
    }

    /// The count of `event` over every query.
    #[must_use]
    pub fn total(&self, event: Event) -> u64 {
        self.rows.iter().map(|(_, counts)| counts[event as usize]).sum()
    }
}

impl Db {
    /// What this database did with each query's memos so far, the queries that did most first.
    #[must_use]
    pub fn tally(&self) -> Tally {
        let mut rows: Vec<_> = self
            .tables
            .iter()
            .map(|(_, table)| (table.name(), table.counts().read()))
            .filter(|(_, counts)| counts.iter().any(|&n| n > 0))
            .collect();
        rows.sort_by_key(|&(name, counts)| (std::cmp::Reverse(counts.iter().sum::<u64>()), name));
        Tally { rows }
    }
}

impl Record for Tally {
    fn kind(&self) -> &'static str {
        "memos"
    }

    /// A row per query and a column per event any query had.
    fn text(&self) -> Text {
        let events: Vec<Event> = Event::ALL.iter().copied().filter(|&e| self.total(e) > 0).collect();
        let columns: Vec<Column> =
            events.iter().map(|e| Column::right(e.name())).chain([Column::left("query")]).collect();
        let rows = self.rows.iter().map(|(name, counts)| {
            let cells = events.iter().map(|&e| Text::from(counts[e as usize].to_string()));
            cells.chain([Text::of(Style::Literal, *name)]).collect::<Vec<_>>()
        });
        let total = events.iter().map(|&e| Text::from(self.total(e).to_string()));
        let total = total.chain([Text::of(Style::Strong, "total")]).collect::<Vec<_>>();
        Text::from("what each query did with its memos")
            .newline()
            .append(Table::new(&columns).rows(rows.chain([total])).text())
    }

    fn fields(&self) -> Object<'static> {
        let rows: Vec<Value<'static>> = self
            .rows
            .iter()
            .map(|(name, counts)| {
                let row = Object::new().with("query", *name);
                let row = Event::ALL.iter().fold(row, |row, &e| row.with(e.name(), counts[e as usize]));
                Value::Object(row)
            })
            .collect();
        Object::new().with("queries", rows)
    }
}

impl Status for Tally {
    fn verb(&self) -> Verb {
        Verb::good("Memos")
    }
}
