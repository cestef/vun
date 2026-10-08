//! Running a program: the global options, the selected command, and a binary's `main`.

use std::io;
use std::process::ExitCode;

use vun_data::Object;
use vun_diag::{Color, DiagCtxt, DiagId, DiagSink, Diagnostic, Msg};
use vun_origin::OriginTable;
use vun_render_json::JsonRenderer;
use vun_renderer::{Renderer, Short};
use vun_span::SourceFiles;
use vun_term::{ColorChoice, Document, Env, Format, Out, Record, Shell, Stream, Text, Verbosity};

use crate::{Action, Args, COLOR, Detail, FORMAT, Outcome, Program, QUIET, Refusal, Request, VERBOSE};

/// A command stopped by an input or output error it did not report itself.
const FAILED: DiagId = DiagId::new("command.failed");

/// The kind of the record a help page is reported as.
const HELP: &str = "help";

/// What a program's context does around a command: collect a refusal as a diagnostic, and
/// report what was collected once the command is done.
pub trait Context {
    fn refused(&mut self, diagnostic: Diagnostic);
    /// A command is about to run: its options are applied, and the shell decides what is shown.
    fn started(&mut self, _shell: &mut Shell<'_>) {}
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
        let mut sink = shell.diagnostics(&Short);
        for d in self.sorted(&origins) {
            sink.emit(d, &origins, &sources);
        }
    }
}

/// Where a shell shows diagnostics.
pub trait Diagnostics {
    /// The sink for diagnostics: `human` on standard error, as text or as lines of pieces, or
    /// JSON lines on standard output.
    fn diagnostics(&mut self, human: &dyn Renderer) -> Box<dyn DiagSink + '_>;

    /// Writes what `human` kept of the run, once it is over, on standard output, where the
    /// document of a format that is one for all diagnostics belongs.
    fn finish_diagnostics(&mut self, human: &dyn Renderer);
}

impl Diagnostics for Shell<'_> {
    fn diagnostics(&mut self, human: &dyn Renderer) -> Box<dyn DiagSink + '_> {
        match self.format() {
            Format::Human => {
                let color = self.err.color();
                human.sink(self.err.writer(), color)
            }
            Format::Json => Box::new(JsonRenderer::new(self.out.writer())),
            // Plain, for the stream to cut into lines of pieces.
            Format::Styled => human.sink(self.err.writer(), Color::Never),
        }
    }

    fn finish_diagnostics(&mut self, human: &dyn Renderer) {
        match self.format() {
            Format::Human | Format::Styled => human.finish(self.out.writer()),
            // Each diagnostic is a line of its own already.
            Format::Json => {}
        }
    }
}

/// The version, as `--version` reports it.
struct Version {
    name: &'static str,
    release: &'static str,
    build: Option<&'static str>,
}

impl Record for Version {
    fn kind(&self) -> &'static str {
        "version"
    }

    fn text(&self) -> Text {
        match self.build {
            Some(build) => Text::from(format!("{} {} ({build})", self.name, self.release)),
            None => Text::from(format!("{} {}", self.name, self.release)),
        }
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("name", self.name).with("version", self.release).with("build", self.build)
    }
}

impl<Cx> Program<Cx> {
    /// Parses `words` against this program and runs the selected command, or prints help or the version.
    ///
    /// # Errors
    /// Returns a refusal for usage errors and for commands that are not implemented yet.
    pub fn run(&self, words: &[String], cx: &mut Cx, shell: &mut Shell<'_>) -> Result<Outcome, Refusal>
    where
        Cx: Context,
    {
        let usage = |diagnostic| Refusal { outcome: Outcome::Usage, diagnostic };
        Self::apply(&self.globals(words), shell);
        let request = self.parse(words).map_err(usage)?;
        let written = match request {
            Request::Help(selected, _, detail) => {
                let page = selected.help(detail);
                match (shell.format(), detail) {
                    (Format::Human | Format::Styled, Detail::Short) => shell.out.print(&page),
                    (Format::Human | Format::Styled, Detail::Long) => shell.page(&page),
                    (Format::Json, _) => {
                        let name = selected.last().name.to_owned();
                        shell.result(&Document { kind: HELP, name, text: page.to_string() })
                    }
                }
                .map(|()| Outcome::Success)
            }
            Request::Version => shell
                .result(&Version { name: self.root.name, release: self.version, build: self.build })
                .map(|()| Outcome::Success),
            Request::Run(selected, mut args) => match selected.last().action {
                Action::Run(run) => {
                    (self.defaults)(cx, &selected, &mut args).map_err(usage)?;
                    (self.configure)(cx, &args).map_err(usage)?;
                    cx.started(shell);
                    run(cx, &args, shell)
                }
                Action::Group(_) => unreachable!("parse turns a bare group into a help request"),
            },
        };
        match written {
            Ok(outcome) => Ok(outcome),
            // A reader that stopped reading, such as `| head`, wants nothing more, not a report.
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(Outcome::Failure),
            Err(e) => {
                let diagnostic =
                    Diagnostic::error(FAILED, Msg::new().text("the command failed")).caused_by(&e);
                Err(Refusal { outcome: Outcome::Failure, diagnostic: Box::new(diagnostic) })
            }
        }
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

    /// Applies the global options to the shell.
    fn apply(args: &Args, shell: &mut Shell<'_>) {
        if let Some(choice) = args.choice(&COLOR, &ColorChoice::ALL, |c| c.name()) {
            shell.choose(*choice);
        }
        if let Some(format) = args.choice(&FORMAT, &Format::ALL, |f| f.name()) {
            shell.show(*format);
        }
        if args.flag(&QUIET) {
            shell.verbosity = Verbosity::Quiet;
        } else if args.flag(&VERBOSE) {
            shell.verbosity = Verbosity::Verbose;
        }
        // The logs and the bars would tear each other.
        shell.animate(std::env::var_os("VUN_LOG").is_some() || args.times(&VERBOSE) > 1);
    }
}
