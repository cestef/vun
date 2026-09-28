//! `vun debug queries`: what the queries saved beside a root file hold, per query and per pool of
//! handles, largest first: what to look at when the saved file grows or loads slowly.

use std::io;
use std::path::Path;

use vun_command::{Args, Outcome};
use vun_diag::{Diagnostic, Msg, errors};
use vun_json::Object;
use vun_query::{Census, Counted};
use vun_term::{Bytes, Column, Record, Row, Shell, Style, Text};

use super::FILE;
use vun_driver::Session;

/// What a counted part of the file is.
#[derive(Clone, Copy)]
enum Part {
    Query,
    Pool,
}

impl Part {
    fn name(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Pool => "pool",
        }
    }
}

/// One counted part, as the table shows it.
struct Held<'a> {
    part: Part,
    counted: &'a Counted,
}

impl Record for Held<'_> {
    fn kind(&self) -> &'static str {
        "held"
    }

    fn text(&self) -> Text {
        Text::join(self.cells(), "  ")
    }

    fn fields(&self) -> Object<'static> {
        let Counted { name, entries, memos, reads, bytes } = self.counted;
        Object::new()
            .with("part", self.part.name())
            .with("name", name.clone())
            .with("entries", *entries)
            .with("memos", *memos)
            .with("reads", *reads)
            .with("bytes", *bytes)
    }
}

impl Row for Held<'_> {
    const COLUMNS: &'static [Column] = &[
        Column::left("part"),
        Column::left("name"),
        Column::right("entries"),
        Column::right("memos"),
        Column::right("reads"),
        Column::right("size"),
    ];

    fn cells(&self) -> Vec<Text> {
        let Counted { name, entries, memos, reads, bytes } = self.counted;
        vec![
            Text::of(Style::Dim, self.part.name()),
            Text::of(Style::Literal, name.clone()),
            Text::from(entries.to_string()),
            Text::from(memos.to_string()),
            Text::from(reads.to_string()),
            Text::from(Bytes(*bytes as u64).to_string()),
        ]
    }
}

pub(super) fn queries(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let root = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(path) = Session::saved_at(Path::new(root)) else { return Ok(Outcome::Failure) };
    let unreadable = |session: &mut Session, cause: &(dyn std::error::Error + 'static)| {
        let msg = Msg::new().text("cannot read the saved queries ").code(path.display().to_string());
        session.diag.error(Diagnostic::error(errors::IO_UNREADABLE_FILE, msg).caused_by(cause));
        Ok(Outcome::Failure)
    };
    let census = match std::fs::read(&path) {
        Ok(bytes) => Census::of(&bytes),
        Err(e) => return unreadable(session, &e),
    };
    let census = match census {
        Ok(census) => census,
        Err(e) => return unreadable(session, &e),
    };
    let mut held: Vec<Held<'_>> =
        census.queries.iter().map(|counted| Held { part: Part::Query, counted }).collect();
    held.extend(census.pools.iter().map(|counted| Held { part: Part::Pool, counted }));
    held.sort_by_key(|h| std::cmp::Reverse(h.counted.bytes));
    shell.table(&held)?;
    Ok(Outcome::Success)
}
