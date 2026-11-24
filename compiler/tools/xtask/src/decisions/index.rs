//! The index of the decisions, `docs/spec/decisions/README.md`: written from the files.

use std::fmt::Write;

use super::entry::Status;
use super::{Decisions, FILE_EXTENSION, QUESTIONS};

/// The spans of numbers below the highest that no decision holds.
fn gaps(numbers: &[u32]) -> Vec<(u32, u32)> {
    let mut spans = Vec::new();
    for pair in numbers.windows(2) {
        if pair[1] > pair[0] + 1 {
            spans.push((pair[0] + 1, pair[1] - 1));
        }
    }
    spans
}

impl Decisions {
    /// The index: what is proposed, then every decision.
    pub(crate) fn index(&self) -> String {
        let numbers: Vec<u32> = self.all().iter().map(|d| d.number).collect();
        let link = |number: u32| format!("[{number}]({})", Self::file_name(number));
        let mut text = format!(
            "# Decisions\n\nOne file per decision, `<number>{FILE_EXTENSION}`: its title, its facts (`Status`, \
             `Where` the spec files that state it, `Amended by`, `Superseded by`, `Note`) and its text. \
             Numbers are never reused, and code cites them as `decision <number>`. The questions the first \
             decisions settled are in [{QUESTIONS}]({QUESTIONS}{FILE_EXTENSION}). This index is written by \
             `cargo xtask decisions`.\n\n"
        );
        let gaps: Vec<String> = gaps(&numbers)
            .into_iter()
            .map(|(from, to)| if from == to { from.to_string() } else { format!("{from} to {to}") })
            .collect();
        if !gaps.is_empty() {
            let _ = write!(text, "Numbers {} were never assigned.\n\n", gaps.join(", "));
        }
        let proposed: Vec<_> = self.all().iter().filter(|d| d.status == Status::Proposed).collect();
        if !proposed.is_empty() {
            text.push_str("## Proposed\n\n| # | Decision | Where |\n|---|----------|-------|\n");
            for d in proposed {
                let _ = writeln!(text, "| {} | {} | {} |", link(d.number), d.title, d.places.join(", "));
            }
            text.push('\n');
        }
        text.push_str("## All\n\n| # | Decision | Status | Where |\n|---|----------|--------|-------|\n");
        for d in self.all() {
            let _ = writeln!(
                text,
                "| {} | {} | {} | {} |",
                link(d.number),
                d.title,
                d.status.name(),
                d.places.join(", ")
            );
        }
        text
    }
}
