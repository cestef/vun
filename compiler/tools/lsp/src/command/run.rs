//! Running the program a root file exports: a lens over each declaration exporting it, and the
//! command building it and running it, its output written beside the document as it comes, then
//! how it ended.

use std::fs::OpenOptions;
use std::io::Write;
use std::process::Stdio;

use vun_data::{FromData, ToData};
use vun_driver::view::View;
use vun_fs::Edit;
use vun_lsp_protocol::base::TextDocumentIdentifier;
use vun_lsp_protocol::command::{CodeLens, CodeLensParams};
use vun_lsp_protocol::requests as r;
use vun_term::Shell;
use vun_trace::tracing;

use super::Order;
use crate::failure::Failure;
use crate::methods::Answers;
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

impl Answers<r::CodeLens> for Server<'_> {
    fn answer(&mut self, params: CodeLensParams) -> Result<Vec<CodeLens>, Failure> {
        let file = self.document(&params.text_document)?.file;
        let ran = Ran { text_document: params.text_document };
        let lenses = self.session.runnable(file).into_iter().filter_map(|name| {
            let range = self.location(name)?.range;
            Some(CodeLens { range, command: Some(Order::Run.command(TITLE.to_owned(), &ran)) })
        });
        Ok(lenses.collect())
    }
}

impl Server<'_> {
    /// Builds the program `ran` names and runs it, its output written beside the document as it
    /// comes, and shown; what stops the build is written there instead.
    pub(super) fn run_program(&mut self, ran: &Ran) -> Result<(), Failure> {
        let document = self.document(&ran.text_document)?;
        let (file, path) = (document.file, View::shown(&document.path, OUTPUT));
        // A new file each run: one still running writes to its own.
        path.put([]).map_err(Failure::unwritten)?;
        let mut output = OpenOptions::new().append(true).open(&path).map_err(Failure::unwritten)?;
        let Ok(program) = self.session.build(file, None) else {
            let mut out = Vec::new();
            self.session.emit(&mut Shell::plain(&mut Vec::new(), &mut out));
            output.write_all(&out).map_err(Failure::unwritten)?;
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
