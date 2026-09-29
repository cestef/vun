//! `cargo xtask hooks`: jj runs no git hooks, so tasks hook in as aliases of this repository's jj
//! configuration instead, each running the task of its name: `jj push -c @-` runs the quick
//! checks, then `jj git push -c @-`.

use std::io;
use std::path::Path;

use vun_command::{Args, Outcome, Shell};
use vun_data::toml::Inline;
use vun_data::{Object, Value};
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Failure, Invocation, Tool};

use crate::task::{self, Program, Task};

/// The tasks jj answers to.
const ALIASED: &[&Task] = &[&task::CHECK, &task::PUSH];

/// What makes jj run an external command, and the command that runs a task.
const EXEC: [&str; 3] = ["util", "exec", "--"];
const XTASK: [&str; 2] = [Program::Cargo.name(), "xtask"];
/// What ends the options, so that the alias's words reach the task.
const END: &str = "--";

/// `jj config set --repo <key> <value>`, with the value read as TOML.
const SET: [&str; 3] = ["config", "set", "--repo"];
const ALIASES: &str = "aliases.";

const UNSET: DiagId = DiagId::new("xtask.hooks.unset");

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
pub(crate) fn install_all(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let jj = match Program::Jj.find() {
        Ok(jj) => jj,
        Err(d) => {
            diag.error(*d);
            return Ok(Outcome::Failure);
        }
    };
    let root = repo_root();
    let mut outcome = Outcome::Success;
    for task in ALIASED {
        match Alias(task).install(&root, &jj) {
            Ok(()) => shell.status(&Installed { name: task.name }),
            Err(failure) => {
                let msg = Msg::new().text("cannot set the jj alias ").code(task.name);
                diag.error(Diagnostic::error(UNSET, msg).caused_by(&failure));
                outcome = Outcome::Failure;
            }
        }
    }
    Ok(outcome)
}

/// The jj alias of a task.
struct Alias<'t>(&'t Task);

impl Alias<'_> {
    /// The words it stands for.
    fn words(&self) -> Vec<&'static str> {
        let end = self.0.passes_words().then_some(END);
        EXEC.into_iter().chain(XTASK).chain([self.0.name]).chain(end).collect()
    }

    /// Sets it in the configuration of the repository at `root`.
    fn install(&self, root: &Path, jj: &Executable) -> Result<(), Failure> {
        let key = Symbol::intern(&[ALIASES, self.0.name].concat());
        let value = Symbol::intern(&Inline(&Value::from(self.words())).to_string());
        let mut args: Vec<Arg> = SET.into_iter().map(Arg::from).collect();
        args.extend([Arg::Text(key), Arg::Text(value)]);
        let inv = Invocation { args, cwd: Some(root.to_owned()), ..Invocation::default() };
        jj.check(&inv).map(drop)
    }
}

/// An alias set.
struct Installed {
    name: &'static str,
}

impl Record for Installed {
    fn kind(&self) -> &'static str {
        "alias"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, [Program::Jj.name(), " ", self.name].concat())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.name)
    }
}

impl Status for Installed {
    fn verb(&self) -> Verb {
        Verb::good("Installed")
    }
}
