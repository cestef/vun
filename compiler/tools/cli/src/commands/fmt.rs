//! `vun fmt [--check] [paths]...`: lays out source files as the formatter does (spec 13,
//! section 5). A path is a file, a directory or a package's directory, as for the other commands;
//! with none, the package around the current directory. A file that does not parse is left as it
//! is, its diagnostics reported.

use std::io;
use std::path::Path;

use vun_command::{Args, Arity, Command, Example, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_fmt::Layout;
use vun_fs::Edit;
use vun_term::{Record, Shell, Status, Style, Text, Verb};

use vun_driver::Session;
use vun_driver::subject::Subject;

const PATHS: Param = Param::positional("paths")
    .arity(Arity::Many)
    .about("files, directories or packages to lay out; the package around this directory if none")
    .values(Values::Path(PathKind::Any));
const WIDTH: Param = Param::option("width")
    .about("the most columns a line has where it can be broken")
    .values(Values::Count)
    .env("VUN_FMT_WIDTH");
const CHECK: Param = Param::flag("check").about("change nothing, and fail when a file would change");

pub(super) const FMT: Command<Session> =
    Command::run("fmt", "lay out source files as the formatter does", fmt)
        .params(&[PATHS, CHECK, WIDTH])
        .examples(&[
            Example { line: "", about: "lay out the package around this directory" },
            Example { line: "main.vun", about: "lay out a file in place" },
            Example { line: "--check net/ lib.vun", about: "fail when a file is not laid out" },
            Example { line: "--width 100 main.vun", about: "break lines past 100 columns" },
        ]);

/// What laying out a file changed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    /// The file was rewritten.
    Written,
    /// The file would change; `--check` left it.
    Pending,
}

/// A file the formatter changed, or would change.
struct Laid {
    path: String,
    change: Change,
}

impl Record for Laid {
    fn kind(&self) -> &'static str {
        "laid_out"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.path.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("path", self.path.clone()).with("written", self.change == Change::Written)
    }
}

impl Status for Laid {
    fn verb(&self) -> Verb {
        match self.change {
            Change::Written => Verb::good("Formatted"),
            Change::Pending => Verb::failed("Unformatted"),
        }
    }
}

fn fmt(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let check = args.flag(&CHECK);
    let width = args.count(&WIDTH).and_then(|w| usize::try_from(w).ok()).unwrap_or(vun_fmt::WIDTH);
    let named: Vec<&Path> = args.values(&PATHS).map(Path::new).collect();
    let mut files = Vec::new();
    for path in if named.is_empty() { vec![None] } else { named.into_iter().map(Some).collect() } {
        let Some(subject) = session.subject(path)? else { return Ok(session.outcome()) };
        files.extend(session.files(&subject));
    }
    let mut pending = false;
    for path in &files {
        let Some(laid) = session.laid_out(path, width) else { continue };
        let change = if check { Change::Pending } else { Change::Written };
        if change == Change::Written {
            path.put(laid)?;
        }
        pending |= change == Change::Pending;
        shell.status(&Laid { path: Subject::relative(path), change });
    }
    Ok(if pending { Outcome::Failure } else { session.outcome() })
}

trait Formatted {
    /// The file at `path` laid out, when that changes it; `None` when it is laid out already, or
    /// does not read or parse, which is reported.
    fn laid_out(&mut self, path: &Path, width: usize) -> Option<String>;
}

impl Formatted for Session {
    fn laid_out(&mut self, path: &Path, width: usize) -> Option<String> {
        let text = self.read(path)?;
        let file = self.sources.add(path.display().to_string(), text);
        let errors = self.diag.error_count();
        let tokens = self.lex(file);
        let cst = self.parse(&tokens);
        if self.diag.error_count() > errors {
            return None;
        }
        let text = self.text(file);
        let laid = Layout::new(&tokens, &cst, text).width(width).to_string();
        (laid != text).then_some(laid)
    }
}
