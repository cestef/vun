//! One decision, a file of `docs/spec/decisions/`: a title line, the facts about it one to a line,
//! then its text.

use std::error::Error;
use std::fmt;

use vun_variants::Variants;

/// Where a decision stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub(crate) enum Status {
    Accepted,
    Proposed,
    Superseded,
}

impl Status {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Proposed => "proposed",
            Self::Superseded => "superseded",
        }
    }

    fn named(word: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|status| status.name() == word)
    }
}

/// What a line before a decision's text states, by the word its line begins with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
enum Fact {
    Status,
    Where,
    AmendedBy,
    SupersededBy,
    Note,
}

impl Fact {
    const fn key(self) -> &'static str {
        match self {
            Self::Status => "Status",
            Self::Where => "Where",
            Self::AmendedBy => "Amended by",
            Self::SupersededBy => "Superseded by",
            Self::Note => "Note",
        }
    }
}

/// A decision's facts, without its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decision {
    pub(crate) number: u32,
    pub(crate) title: String,
    pub(crate) status: Status,
    /// The spec files that state it, by their number.
    pub(crate) places: Vec<String>,
    pub(crate) amended_by: Vec<u32>,
    pub(crate) superseded_by: Option<u32>,
    pub(crate) note: Option<String>,
}

/// Why a decision's file is not read.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Malformed {
    /// The first line is not `# <number>. <title>`.
    Title,
    /// A line before the text begins with no known fact.
    UnknownFact {
        line: usize,
    },
    /// A fact that takes a status, a number or numbers holds something else.
    Value {
        line: usize,
    },
    /// A fact stated twice.
    Repeated(&'static str),
    NoStatus,
    /// A superseded decision names no successor, or another one names a successor.
    Succession,
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Title => write!(f, "the first line is not `# <number>. <title>`"),
            Self::UnknownFact { line } => write!(f, "line {line} states no known fact"),
            Self::Value { line } => write!(f, "line {line} holds a value its fact does not take"),
            Self::Repeated(key) => write!(f, "`{key}` is stated twice"),
            Self::NoStatus => write!(f, "no status is stated"),
            Self::Succession => write!(f, "only a superseded decision names the one that supersedes it"),
        }
    }
}

impl Error for Malformed {}

impl Decision {
    /// The decision `text` holds.
    ///
    /// # Errors
    /// Fails when the title or a fact is malformed.
    pub(crate) fn parse(text: &str) -> Result<Self, Malformed> {
        let mut lines = text.lines().enumerate();
        let (_, head) = lines.next().ok_or(Malformed::Title)?;
        let (number, title) = head
            .strip_prefix("# ")
            .and_then(|rest| rest.split_once(". "))
            .and_then(|(number, title)| Some((number.parse().ok()?, title.trim().to_owned())))
            .filter(|(_, title)| !title.is_empty())
            .ok_or(Malformed::Title)?;
        let mut decision = Self {
            number,
            title,
            status: Status::Accepted,
            places: Vec::new(),
            amended_by: Vec::new(),
            superseded_by: None,
            note: None,
        };
        let mut seen = Vec::new();
        let mut status = None;
        for (index, line) in lines {
            if line.is_empty() {
                if seen.is_empty() {
                    continue;
                }
                break;
            }
            let line_number = index + 1;
            let (key, value) = line.split_once(": ").ok_or(Malformed::UnknownFact { line: line_number })?;
            let fact = Fact::ALL
                .iter()
                .copied()
                .find(|fact| fact.key() == key)
                .ok_or(Malformed::UnknownFact { line: line_number })?;
            if seen.contains(&fact) {
                return Err(Malformed::Repeated(fact.key()));
            }
            seen.push(fact);
            let value_error = || Malformed::Value { line: line_number };
            match fact {
                Fact::Status => status = Some(Status::named(value).ok_or_else(value_error)?),
                Fact::Where => decision.places = value.split(", ").map(str::to_owned).collect(),
                Fact::AmendedBy => decision.amended_by = Self::numbers(value).ok_or_else(value_error)?,
                Fact::SupersededBy => {
                    decision.superseded_by = Some(value.parse().map_err(|_| value_error())?);
                }
                Fact::Note => decision.note = Some(value.to_owned()),
            }
        }
        decision.status = status.ok_or(Malformed::NoStatus)?;
        if (decision.status == Status::Superseded) != decision.superseded_by.is_some() {
            return Err(Malformed::Succession);
        }
        Ok(decision)
    }

    fn numbers(value: &str) -> Option<Vec<u32>> {
        value.split(", ").map(|number| number.parse().ok()).collect()
    }
}
