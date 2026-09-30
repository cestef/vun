//! Running the program a root file exports: a lens over each declaration exporting it, and the
//! command building it and running it, its output written beside the document as it comes, then
//! how it ended.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use vun_data::{FromData, ToData};
use vun_fs::Edit;
use vun_lsp_protocol::base::TextDocumentIdentifier;
use vun_lsp_protocol::command::CodeLens;
use vun_term::Shell;
use vun_trace::tracing;

use super::Order;
use crate::failure::Failure;
use crate::server::Server;

/// What the output of a run is named by, beside the document.
const OUTPUT: &str = "run";
const TITLE: &str = "Run";

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

    /// A lens over each declaration of `document` exporting the program it roots.
    pub(super) fn run_lenses(&mut self, document: &TextDocumentIdentifier) -> Result<Vec<CodeLens>, Failure> {
        let file = self.document(document)?.file;
        let ran = Ran { text_document: document.clone() };
        let lenses = self.session.runnable(file).into_iter().filter_map(|name| {
            let range = self.location(name)?.range;
            Some(CodeLens { range, command: Some(Order::Run.command(TITLE.to_owned(), &ran)) })
        });
        Ok(lenses.collect())
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
        let mut command = self.session.command(&program);
        command.stdin(Stdio::null()).stdout(output.try_clone().map_err(Failure::unwritten)?);
        let mut child = command.stderr(errors).spawn().map_err(Failure::unwritten)?;
        std::thread::spawn(move || {
            let ended = child.wait().and_then(|status| writeln!(output, "\n{status}"));
            if let Err(error) = ended {
                tracing::warn!(%error, "a run did not end cleanly");
            }
        });
        self.show(&path)
    }
}
