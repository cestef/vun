//! What a profile reports: the run's total, then what the run spent it on, ranked.

use vun_data::{Object, ToData};
use vun_driver::testing::{Comparison, Statistics, Verdict};
use vun_term::{Column, Noun, Percent, Record, Row, Status, Style, Text, Verb};

use crate::entry::{Entry, Key, Scope};

const RUNS: Noun = Noun { one: "run", many: "runs" };

/// Characters of a subject shown to people; the data holds it whole.
const SHOWN: usize = 96;

/// A quantity against the baseline's.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Against {
    pub comparison: Comparison,
}

impl Against {
    fn text(self) -> Text {
        let style = match self.comparison.verdict {
            Verdict::Regressed => Style::Error,
            Verdict::Improved => Style::Good,
            Verdict::Unchanged => Style::Dim,
        };
        Text::of(style, format!("{:+.1}%", self.comparison.change))
    }

    fn fields(self, object: Object<'static>) -> Object<'static> {
        object
            .with("change", self.comparison.change.to_data())
            .with("verdict", self.comparison.verdict.to_data())
    }
}

/// What a subject and its callees ran.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Callees {
    /// The subject has no callees.
    #[default]
    Unknown,
    /// It reaches itself through others: no cost is its own.
    Cycle,
    Ran(f64),
}

/// What a subject adds to its quantity: what it and its callees ran, and how often it ran.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Extra {
    pub callees: Callees,
    pub count: Option<u64>,
}

/// One subject of a run, with its share of the run.
pub(crate) struct Ranked {
    pub entry: Entry,
    /// Of the quantity of the whole run.
    pub share: Percent,
    pub extra: Extra,
    pub against: Option<Against>,
}

impl Ranked {
    fn subject(&self) -> Text {
        let name = &self.entry.key.name;
        let shown = match name.char_indices().nth(SHOWN) {
            Some((end, _)) => format!("{}...", &name[..end]),
            None => name.clone(),
        };
        Text::of(Style::Literal, shown)
    }

    fn quantity(&self, value: f64) -> Text {
        Text::from(self.entry.key.measure.show(value))
    }
}

impl Record for Ranked {
    fn kind(&self) -> &'static str {
        "profile"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let Key { workload, phase, measure, scope, name } = &self.entry.key;
        let object = Object::new()
            .with("workload", workload.clone())
            .with("phase", phase.to_data())
            .with("measure", measure.to_data())
            .with("scope", scope.to_data())
            .with("name", name.clone())
            .with("statistics", self.entry.statistics.to_data())
            .with("share", self.share.0.to_data());
        let object = match self.extra.callees {
            Callees::Ran(inclusive) => object.with("inclusive", inclusive.to_data()),
            Callees::Cycle => object.with("cycle", true),
            Callees::Unknown => object,
        };
        let object = match self.extra.count {
            Some(count) => object.with("count", count),
            None => object,
        };
        self.against.map_or(object.clone(), |a| a.fields(object))
    }
}

impl Row for Ranked {
    const COLUMNS: &'static [Column] = &[
        Column::right("own"),
        Column::right("share"),
        Column::right("with callees"),
        Column::right("runs"),
        Column::right("spread"),
        Column::right("change"),
        Column::left("subject"),
    ];

    fn cells(&self) -> Vec<Text> {
        let Statistics { median, .. } = self.entry.statistics;
        let blank = || Text::of(Style::Dim, "-");
        vec![
            self.quantity(median),
            Text::from(self.share.to_string()),
            match self.extra.callees {
                Callees::Ran(v) => self.quantity(v),
                Callees::Cycle => Text::of(Style::Dim, "cycle"),
                Callees::Unknown => blank(),
            },
            self.extra.count.map_or_else(blank, |n| Text::from(n.to_string())),
            if self.entry.statistics.samples > 1 {
                Text::from(self.entry.statistics.spread().to_string())
            } else {
                blank()
            },
            self.against.map_or_else(blank, Against::text),
            self.subject(),
        ]
    }
}

/// A whole run's quantity.
pub(crate) struct Total {
    pub entry: Entry,
    pub against: Option<Against>,
}

impl Total {
    fn title(&self) -> Text {
        let Key { workload, phase, scope, .. } = &self.entry.key;
        let what = if *scope == Scope::Work { "work" } else { "total" };
        Text::of(Style::Strong, format!("{workload} {}", phase.name())).plain(format!(" {what}"))
    }
}

impl Record for Total {
    fn kind(&self) -> &'static str {
        "profile_total"
    }

    fn text(&self) -> Text {
        let Statistics { median, samples, .. } = self.entry.statistics;
        let shown = self.entry.key.measure.show(median);
        let said = if samples > 1 {
            format!("{shown} over {}, spread {}", RUNS.count(samples), self.entry.statistics.spread())
        } else {
            format!("{shown} in {}", RUNS.count(samples))
        };
        let text = self.title().plain(": ").plain(said);
        match self.against {
            Some(against) => text.plain(", ").append(against.text()),
            None => text,
        }
    }

    fn fields(&self) -> Object<'static> {
        let Key { workload, phase, measure, scope, .. } = &self.entry.key;
        let object = Object::new()
            .with("workload", workload.clone())
            .with("phase", phase.to_data())
            .with("measure", measure.to_data())
            .with("scope", scope.to_data())
            .with("statistics", self.entry.statistics.to_data());
        self.against.map_or(object.clone(), |a| a.fields(object))
    }
}

impl Status for Total {
    fn verb(&self) -> Verb {
        match self.against.map(|a| a.comparison.verdict) {
            Some(Verdict::Regressed) => Verb::failed("Regressed"),
            Some(Verdict::Improved) => Verb::good("Improved"),
            Some(Verdict::Unchanged) | None => Verb::good("Measured"),
        }
    }
}

/// A function's calls to another, for the callees that cost most.
pub(crate) struct Calling {
    pub workload: String,
    pub caller: String,
    pub callee: String,
    pub calls: u64,
    /// None where caller and callee call each other in a cycle.
    pub cost: Option<u64>,
    /// Of what the callee ran with its callees.
    pub share: Option<Percent>,
}

impl Record for Calling {
    fn kind(&self) -> &'static str {
        "profile_calling"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("workload", self.workload.clone())
            .with("caller", self.caller.clone())
            .with("callee", self.callee.clone())
            .with("calls", self.calls)
            .with("cost", self.cost)
            .with("share", self.share.map(|p| p.0.to_data()))
    }
}

impl Row for Calling {
    const COLUMNS: &'static [Column] = &[
        Column::right("cost"),
        Column::right("share"),
        Column::right("calls"),
        Column::left("callee"),
        Column::left("called from"),
    ];

    fn cells(&self) -> Vec<Text> {
        let blank = || Text::of(Style::Dim, "cycle");
        #[expect(clippy::cast_precision_loss, reason = "shown with two decimals")]
        let cost = self.cost.map_or_else(blank, |c| Text::from(vun_term::Instructions(c as f64).to_string()));
        let shown = |name: &str| match name.char_indices().nth(SHOWN / 2) {
            Some((end, _)) => format!("{}...", &name[..end]),
            None => name.to_owned(),
        };
        vec![
            cost,
            self.share.map_or_else(|| Text::of(Style::Dim, "-"), |p| Text::from(p.to_string())),
            Text::from(self.calls.to_string()),
            Text::of(Style::Literal, shown(&self.callee)),
            Text::of(Style::Literal, shown(&self.caller)),
        ]
    }
}
