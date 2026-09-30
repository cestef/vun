//! `vun fix [--unsafe] <file>`: applies the fixes the diagnostics of checking a file propose, to
//! the files of its package, checking again to a fixed point (spec 10, section 6), then reports
//! what is left.

use std::io;
use std::path::PathBuf;
use std::time::Instant;

use vun_command::{Args, Command, Example, Outcome, Param};
use vun_data::Object;
use vun_driver::Session;
use vun_driver::fix::{Applied, Fixed};
use vun_fs::Edit;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use super::status::{Conclude, Started, Step};
use super::{COMPILE, FILE, Inputs};

const UNSAFE: Param =
    Param::flag("unsafe").about("also apply the fixes that may change what the program does");

pub(super) const FIX: Command<Session> =
    Command::run("fix", "apply the fixes diagnostics propose, then report what is left", fix)
        .params(&[FILE, UNSAFE])
        .sets(&[COMPILE])
        .examples(&[
            Example { line: "main.vun", about: "apply the fixes that keep what the program means" },
            Example { line: "--unsafe main.vun", about: "apply those that may change it as well" },
        ]);

/// A file fixes changed.
struct Written {
    path: PathBuf,
    fixes: usize,
}

impl Record for Written {
    fn kind(&self) -> &'static str {
        "fixed"
    }

    fn text(&self) -> Text {
        let fixes = if self.fixes == 1 { " fix" } else { " fixes" };
        Text::of(Style::Strong, self.path.display().to_string())
            .plain(", ")
            .plain(self.fixes.to_string())
            .plain(fixes)
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("path", self.path.display().to_string()).with("fixes", self.fixes)
    }
}

impl Status for Written {
    fn verb(&self) -> Verb {
        Verb::good("Fixed")
    }
}

fn fix(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let started = Instant::now();
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    shell.status(&Started { step: Step::Fix, subject: path.to_owned() });
    let applied = if args.flag(&UNSAFE) { Applied::Unsafe } else { Applied::Safe };
    if let Some(file) = session.root(args, shell) {
        for Fixed { path, fixes, text } in session.fix(file, applied) {
            path.put(text)?;
            shell.status(&Written { path, fixes });
        }
    }
    session.conclude(shell, path.to_owned(), started);
    Ok(session.outcome())
}
