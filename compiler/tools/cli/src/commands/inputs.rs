//! What a command acts on: the path it names resolved to a subject, its project made ready, and
//! the programs it builds (decision 356).

use std::io;
use std::path::Path;
use std::time::Instant;

use vun_build::layout::MANIFEST;
use vun_command::Args;
use vun_driver::Built;
use vun_driver::Session;
use vun_driver::subject::Subject;
use vun_span::FileId;
use vun_term::Shell;

use super::PATH;
use super::status::{Conclude, Report, Started, Step};

/// What a command's path names, once its project is ready, and how it is shown.
pub(super) struct Named {
    pub(super) subject: Subject,
    pub(super) shown: String,
    /// Whether the project is ready: its dependencies fetched and its lockfile written.
    pub(super) ready: bool,
}

pub(super) trait Inputs {
    /// What the path `args` gives names, with none the package around this directory, for `step`:
    /// the command handed to the toolchain the project pins if it pins another, `step` announced,
    /// then the project made ready; `None` once why nothing is named is reported.
    fn named(&mut self, args: &Args, step: Step, shell: &mut Shell<'_>) -> io::Result<Option<Named>>;

    /// Makes the project around the file at `path` ready, as cargo does before compiling:
    /// dependencies fetched and the lockfile written, after handing the command to the
    /// toolchain the project pins if it pins another. `started` is shown once this toolchain is
    /// the one to run, so a handed-off command announces it once.
    fn ready(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> bool;

    /// Reads the file at `path` once its project is ready.
    fn root_at(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> Option<FileId>;

    /// Builds what the path `args` gives names, reporting how it went: the program of a file, or
    /// the programs the plan of each package records.
    fn built(&mut self, args: &Args, shell: &mut Shell<'_>) -> io::Result<Option<Vec<Built>>>;
}

impl Inputs for Session {
    fn named(&mut self, args: &Args, step: Step, shell: &mut Shell<'_>) -> io::Result<Option<Named>> {
        let path = args.value(&PATH).map(Path::new);
        let Some(subject) = self.subject(path)? else { return Ok(None) };
        if !self.resolve_lints(&subject) {
            return Ok(None);
        }
        let shown = path.map_or_else(|| subject.shown(), |p| p.display().to_string());
        let ready = self.ready(&subject.root(), Some(&Started { step, subject: shown.clone() }), shell);
        Ok(Some(Named { subject, shown, ready }))
    }

    fn ready(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> bool {
        // A project pinning another toolchain is that one's to build.
        if self.hand_off(path).is_err() {
            return false;
        }
        if let Some(started) = started {
            shell.status(started);
        }
        for (name, hash, fetched) in self.prepare(path) {
            fetched.report(name, hash, shell);
        }
        !self.failed()
    }

    fn root_at(&mut self, path: &Path, started: Option<&Started>, shell: &mut Shell<'_>) -> Option<FileId> {
        self.ready(path, started, shell).then(|| self.load(path)).flatten()
    }

    fn built(&mut self, args: &Args, shell: &mut Shell<'_>) -> io::Result<Option<Vec<Built>>> {
        let started = Instant::now();
        let Some(Named { subject, shown, ready }) = self.named(args, Step::Build, shell)? else {
            return Ok(None);
        };
        let output = args.value(&vun_driver::program::OUTPUT).map(Path::new);
        let built = if !ready {
            None
        } else if let Some(file) = subject.file() {
            // A program its package's plan records is built as the plan says; any other file
            // builds as a program of its own.
            let planned = match &subject {
                Subject::Package { dir, .. } => {
                    self.load(&dir.join(MANIFEST)).and_then(|plan| self.planned(plan, file, output).ok())
                }
                Subject::Loose(_) | Subject::Workspace { .. } => Some(None),
            };
            planned.and_then(|planned| match planned {
                Some(program) => Some(vec![program]),
                None => self.load(file).and_then(|file| self.build_alone(file, output).ok()).map(|p| vec![p]),
            })
        } else {
            // A package's plan records its programs; a workspace builds each package's.
            let plans = subject.packages().into_iter().map(|dir| {
                let manifest = self.load(&dir.join(MANIFEST))?;
                self.carry_out(manifest).ok()
            });
            plans.collect::<Option<Vec<_>>>().map(|built| built.concat())
        };
        let concluded = match built.as_deref() {
            Some([program]) => Subject::relative(&program.path),
            _ => shown,
        };
        self.conclude(shell, concluded, started);
        Ok(built)
    }
}
