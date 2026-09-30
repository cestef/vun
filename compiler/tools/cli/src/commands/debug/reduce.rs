//! `vun debug reduce <file> [--keep <error>]`: the least text that fails as the file does,
//! crashing the compiler at the same place or reporting the same error.

use std::borrow::Cow;
use std::io;

use vun_command::{Args, Outcome, Param};
use vun_data::Object;
use vun_diag::{DiagId, errors};
use vun_reduce::Stats;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use super::FILE;
use super::dump::DUMP;
use vun_driver::Session;

pub(super) const KEEP: Param = Param::option("keep")
    .about("the error the reduced file must still report; the file's first failure otherwise")
    .values(vun_command::Values::Catalog(vun_command::Catalog {
        names: || {
            let relative = |id: &DiagId| match id.0 {
                Cow::Borrowed(path) => path.strip_prefix(DiagId::COMPILER),
                Cow::Owned(_) => None,
            };
            errors::ALL.iter().filter_map(relative).collect()
        },
        what: "an error",
        declared_in: errors::DECLARED_IN,
    }));

/// How far a file was cut down.
struct Cut {
    path: String,
    stats: Stats,
    after: usize,
}

impl Record for Cut {
    fn kind(&self) -> &'static str {
        "reduced"
    }

    fn text(&self) -> Text {
        let Stats { tests, before, .. } = self.stats;
        Text::of(Style::Strong, self.path.clone())
            .plain(format!(" from {before} to {} bytes, {tests} candidates tested", self.after))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("path", self.path.clone())
            .with("before", self.stats.before)
            .with("after", self.after)
            .with("tests", self.stats.tests)
            .with("kept", self.stats.kept)
    }
}

impl Status for Cut {
    fn verb(&self) -> Verb {
        Verb::good("Reduced")
    }
}

pub(super) fn reduce(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(std::path::Path::new(path)) else { return Ok(session.outcome()) };
    let keep = args.value(&KEEP).map(DiagId::compiler);
    let Ok(reduced) = session.reduce(file, keep) else { return Ok(session.outcome()) };
    shell.status(&Cut { path: path.to_owned(), stats: reduced.stats, after: reduced.text.len() });
    shell.document(DUMP, "reduced".to_owned(), |out| out.write_all(reduced.text.as_bytes()))?;
    Ok(Outcome::Success)
}
