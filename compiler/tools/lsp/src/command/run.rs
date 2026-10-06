//! Running the program a root file exports: a lens over each declaration exporting it, and the
//! command building it and running it, its output written beside the document as it comes, then
//! how it ended. What it reads is a file beside the document too, which a second lens opens.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use vun_data::{FromData, ToData};
use vun_fs::Edit;
use vun_lsp_protocol::base::TextDocumentIdentifier;
use vun_lsp_protocol::command::CodeLens;
use vun_term::Shell;

use super::Order;
use crate::failure::Failure;
use crate::server::Server;

/// What the output of a run is named by, beside the document.
const OUTPUT: &str = "run";
const TITLE: &str = "Run";

/// What the input of a run is named by, beside the document: the program reads it as its standard
/// input, and has none when the file does not exist.
const INPUT: &str = "input";
const INPUT_TITLE: &str = "Input";

/// The arguments of [`Order::Run`].
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub(crate) struct Ran {
    pub(crate) text_document: TextDocumentIdentifier,
}

impl Server<'_> {
    /// An empty file at `path` for what a command writes: a new one each time, since one still
    /// running writes to its own.
    pub(super) fn output(path: &Path) -> Result<File, Failure> {
        path.put([]).map_err(Failure::unwritten)?;
        OpenOptions::new().append(true).open(path).map_err(Failure::unwritten)
    }

    /// Writes to `output` what stopped a build.
    pub(super) fn stopped(&mut self, output: &mut File) -> Result<(), Failure> {
        let mut out = Vec::new();
        self.session.emit(&mut Shell::plain(&mut Vec::new(), &mut out));
        output.write_all(&out).map_err(Failure::unwritten)
    }

    /// A lens over each declaration of `document` exporting the program it roots, one running it
    /// and one opening its input, and, while any runs, one stopping them.
    pub(super) fn run_lenses(&mut self, document: &TextDocumentIdentifier) -> Result<Vec<CodeLens>, Failure> {
        let file = self.document(document)?.file;
        let ran = Ran { text_document: document.clone() };
        let runs = self.session.runnable(file).into_iter().filter_map(|name| {
            let range = self.location(name)?.range;
            Some(CodeLens { range, command: Some(Order::Run.command(TITLE.to_owned(), &ran)) })
        });
        let runs: Vec<_> = runs.collect();
        let inputs = runs.iter().map(|run| {
            let command = Order::Input.command(INPUT_TITLE.to_owned(), &ran);
            CodeLens { range: run.range, command: Some(command) }
        });
        let inputs: Vec<_> = inputs.collect();
        let stops: Vec<_> = runs.iter().filter_map(|run| self.stop_lens(run, document)).collect();
        Ok(runs.into_iter().chain(inputs).chain(stops).collect())
    }

    /// The input of the run of `document`'s program: its file, or none when there is none.
    fn input(&mut self, source: &Path) -> Result<Stdio, Failure> {
        match File::open(self.session.shown_at(source, INPUT)) {
            Ok(file) => Ok(Stdio::from(file)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Stdio::null()),
            Err(error) => Err(Failure::unwritten(error)),
        }
    }

    /// Asks the client to show the input of the run of `ran`'s document, an empty file where it
    /// has none yet.
    pub(super) fn show_input(&mut self, ran: &Ran) -> Result<(), Failure> {
        let source = self.document(&ran.text_document)?.path.clone();
        let path = self.session.shown_at(&source, INPUT);
        if !path.exists() {
            path.put([]).map_err(Failure::unwritten)?;
        }
        self.show(&path)
    }

    /// Builds the program `ran` names and runs it, its output written beside the document as it
    /// comes, and shown; what stops the build is written there instead.
    pub(super) fn run_program(&mut self, ran: &Ran) -> Result<(), Failure> {
        let document = self.document(&ran.text_document)?;
        let (file, source) = (document.file, document.path.clone());
        let path = self.session.shown_at(&source, OUTPUT);
        let mut output = Self::output(&path)?;
        let Ok(program) = self.session.build(file, None) else {
            self.stopped(&mut output)?;
            return self.show(&path);
        };
        drop(std::mem::take(&mut self.session.diag));
        let errors = output.try_clone().map_err(Failure::unwritten)?;
        let stdin = self.input(&source)?;
        let mut command = self.session.command(&program);
        command.stdin(stdin).stdout(output.try_clone().map_err(Failure::unwritten)?);
        let child = command.stderr(errors).spawn().map_err(Failure::unwritten)?;
        self.follow(child, output);
        self.show(&path)
    }
}
