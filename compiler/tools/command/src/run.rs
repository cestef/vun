//! Running a program: the global options, the selected command, and a binary's `main`.

use std::io::{self, Write};
use std::process::ExitCode;

use vun_diag::{DiagCtxt, DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_render_short::ShortRenderer;
use vun_span::SourceFiles;
use vun_term::{ColorChoice, Env, Out, Shell, Stream, Verbosity};

use crate::{Action, Args, COLOR, Detail, Outcome, Program, QUIET, Refusal, Request};

/// What a program's context does around a command: collect a refusal as a diagnostic, and
/// report what was collected once the command is done.
pub trait Context {
    fn refused(&mut self, diagnostic: Diagnostic);
    /// Reports the diagnostics collected, and whatever else is shown at exit.
    fn finish(&mut self, shell: &mut Shell<'_>);
}

/// A program whose context is its diagnostics alone, such as `cargo xtask`: they are about
/// the repository, not a source file, so they show on one line each.
impl Context for DiagCtxt {
    fn refused(&mut self, diagnostic: Diagnostic) {
        self.error(diagnostic);
    }

    fn finish(&mut self, shell: &mut Shell<'_>) {
        let (origins, sources) = (OriginTable::new(), SourceFiles::new());
        let color = shell.err.color();
        let mut renderer = ShortRenderer::new(shell.err.writer(), color);
        for d in self.sorted(&origins) {
            renderer.emit(d, &origins, &sources);
        }
    }
}

impl<Cx> Program<Cx> {
    /// Parses `words` against this program and runs the selected command, or prints help or the version.
    ///
    /// # Errors
    /// Returns a refusal for usage errors and for commands that are not implemented yet.
    pub fn run(&self, words: &[String], cx: &mut Cx, shell: &mut Shell<'_>) -> Result<Outcome, Refusal> {
        let usage = |diagnostic| Refusal { outcome: Outcome::Usage, diagnostic };
        let request = self.parse(words).map_err(usage)?;
        let written = match request {
            Request::Help(selected, args, detail) => {
                Self::apply(&args, shell);
                let page = selected.help(detail);
                match detail {
                    Detail::Short => shell.out.print(&page),
                    Detail::Long => shell.page(&page),
                }
                .map(|()| Outcome::Success)
            }
            Request::Version => {
                writeln!(shell.out, "{} {}", self.root.name, self.version).map(|()| Outcome::Success)
            }
            Request::Run(selected, args) => match selected.last().action {
                Action::Run(run) => {
                    Self::apply(&args, shell);
                    (self.configure)(cx, &args).map_err(usage)?;
                    run(cx, &args, shell)
                }
                Action::Group(_) => unreachable!("parse turns a bare group into a help request"),
            },
        };
        // Output that cannot be written, such as a closed pipe, fails the command without a report.
        Ok(written.unwrap_or(Outcome::Failure))
    }

    /// Runs the program on the process's arguments and standard streams, as a binary's `main`.
    pub fn main(&self, cx: &mut Cx) -> ExitCode
    where
        Cx: Context,
    {
        let words: Vec<String> = std::env::args().skip(1).collect();
        // Standard error stays unlocked: threads log to it while the command runs.
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr());
        let out = Out::new(&mut stdout, Stream::Stdout.is_terminal());
        let err = Out::new(&mut stderr, Stream::Stderr.is_terminal());
        let mut shell = Shell::new(out, err, Env::read());
        let outcome = self.run(&words, cx, &mut shell).unwrap_or_else(|refusal| {
            cx.refused(*refusal.diagnostic);
            refusal.outcome
        });
        cx.finish(&mut shell);
        outcome.into()
    }

    /// Applies the global options to the shell; parsing checked their values.
    fn apply(args: &Args, shell: &mut Shell<'_>) {
        if let Some(choice) = args.choice(&COLOR, &ColorChoice::ALL, |c| c.name()) {
            shell.choose(*choice);
        }
        if args.flag(&QUIET) {
            shell.verbosity = Verbosity::Quiet;
        }
    }
}
