//! The tasks of the repository, declared as data: what each one does, and the tasks it needs
//! first. A task runs its needs in order, every one of them even after one fails, so a run
//! reports all it finds, and does its own work only when they all passed. The commands of
//! `cargo xtask` and the jj aliases are made from this table.

mod call;
mod program;
mod table;

use std::io;

use vun_command::{Args, Arity, Outcome, Param, Run, Shell};
use vun_data::Object;
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Failure, Invocation, Tool};

pub(crate) use call::Call;
pub(crate) use program::Program;
pub(crate) use table::*;

pub(crate) struct Task {
    pub(crate) name: &'static str,
    pub(crate) about: &'static str,
    pub(crate) params: &'static [Param],
    pub(crate) needs: &'static [&'static Task],
    pub(crate) work: Work,
}

/// What a task does once its needs passed.
pub(crate) enum Work {
    /// Nothing more: the task names its needs together.
    Needs,
    /// A command of this program.
    Run(Run<DiagCtxt>),
    Call(Call),
}

/// The words a task passes on to the program it calls.
pub(crate) const WORDS: Param =
    Param::positional("words").arity(Arity::Many).about("passed on to the program the task calls");

const FAILED: DiagId = DiagId::new("xtask.task.failed");

impl Task {
    pub(crate) const fn new(name: &'static str, about: &'static str, work: Work) -> Self {
        Self { name, about, params: &[], needs: &[], work }
    }

    pub(crate) const fn needs(self, needs: &'static [&'static Task]) -> Self {
        Self { needs, ..self }
    }

    pub(crate) const fn params(self, params: &'static [Param]) -> Self {
        Self { params, ..self }
    }

    /// Whether it passes on the words it is given.
    pub(crate) fn passes_words(&self) -> bool {
        self.params.iter().any(|p| p.is(&WORDS))
    }

    /// Runs its needs, then its work if they passed.
    ///
    /// # Errors
    /// Returns an error if the shell cannot be written.
    pub(crate) fn run(&self, diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
        let mut outcome = Outcome::Success;
        for need in self.needs {
            if need.run(diag, args, shell)? == Outcome::Failure {
                outcome = Outcome::Failure;
            }
        }
        if outcome == Outcome::Failure {
            return Ok(outcome);
        }
        match self.work {
            Work::Needs => Ok(Outcome::Success),
            Work::Run(run) => run(diag, args, shell),
            Work::Call(call) => Ok(self.call(call, diag, args, shell)),
        }
    }

    fn call(&self, call: Call, diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> Outcome {
        let program = match call.program().find() {
            Ok(program) => program,
            Err(d) => {
                diag.error(*d);
                return Outcome::Failure;
            }
        };
        let root = repo_root();
        let mut words: Vec<Arg> = call.words().iter().copied().map(Arg::from).collect();
        if self.passes_words() {
            words.extend(args.values(&WORDS).map(|w| Arg::Text(Symbol::intern(w))));
        }
        let inv =
            Invocation { args: words, cwd: Some(root), streams: call.streams(), ..Invocation::default() };
        let failure = match program.run(&inv) {
            Ok(out) => match out.failure() {
                None => {
                    shell.status(&Ran { task: self.name });
                    return Outcome::Success;
                }
                Some(failure) => self.failed(call, &out.stdout).caused_by(&failure),
            },
            Err(e) => self.failed(call, &[]).caused_by(&Failure::Start(e)),
        };
        diag.error(failure);
        Outcome::Failure
    }

    /// The diagnostic of a call that failed: each line it printed is a finding.
    fn failed(&self, call: Call, stdout: &[u8]) -> Diagnostic {
        let root = repo_root();
        let msg = Msg::new().text("the task ").code(self.name).text(" failed");
        let stdout = String::from_utf8_lossy(stdout);
        let found = stdout.lines().filter(|l| !l.trim().is_empty()).map(|line| {
            let path = std::path::Path::new(line);
            path.strip_prefix(&root).unwrap_or(path).display().to_string()
        });
        let d =
            found.fold(Diagnostic::error(FAILED, msg), |d, f| d.child(ChildKind::Note, Msg::new().code(f)));
        match call.fix() {
            Some(fix) => d.child(ChildKind::Help, Msg::new().text("run ").code(fix.line())),
            None => d,
        }
    }
}

/// A task whose call passed.
struct Ran {
    task: &'static str,
}

impl Record for Ran {
    fn kind(&self) -> &'static str {
        "task"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.task)
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("task", self.task)
    }
}

impl Status for Ran {
    fn verb(&self) -> Verb {
        Verb::good("Passed")
    }
}
