//! The commands the server carries out, one row of [`Order`] each, offered as code actions and
//! code lenses: a view of a document, running the program a root file exports and running a
//! test, each written beside the document and shown (spec 13, section 11).

pub(crate) mod run;
pub(crate) mod stop;
pub(crate) mod test;
pub(crate) mod view;

use std::path::Path;

use vun_data::{FromData, ToData, Value};
use vun_lsp_protocol::base::Uri;
use vun_lsp_protocol::command::{
    CodeLens, CodeLensParams, Command, ExecuteCommandParams, ShowDocumentParams,
};
use vun_lsp_protocol::requests as r;
use vun_lsp_protocol::rpc::ErrorCode;
use vun_variants::Variants;

use crate::failure::{Failure, Reason};
use crate::methods::Answers;
use crate::server::Server;

/// A command, by the name the protocol gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants, ToData, FromData)]
pub(crate) enum Order {
    /// Shows a view of a document: [`view::Viewed`].
    #[data(name = "vun.view")]
    View,
    /// Builds and runs the program a document exports: [`run::Ran`].
    #[data(name = "vun.run")]
    Run,
    /// Shows the file a run reads as its standard input, an empty one where there is none:
    /// [`run::Ran`].
    #[data(name = "vun.input")]
    Input,
    /// Builds the tests' program of a document's namespace and runs one test: [`test::Tested`].
    #[data(name = "vun.test")]
    Test,
    /// Ends the programs `vun.run` started that are still running; the arguments are those of
    /// the run's own lens.
    #[data(name = "vun.stop")]
    Stop,
}

impl Order {
    /// The protocol's name of it.
    pub(crate) fn name(self) -> String {
        self.to_data().as_str().map(str::to_owned).unwrap_or_default()
    }

    /// The command carrying it out with `arguments`, titled `title`.
    fn command(self, title: String, arguments: &impl ToData) -> Command {
        Command { title, command: self.name(), arguments: Some(vec![arguments.to_data()]) }
    }
}

impl Failure {
    /// What a command shows could not be written.
    fn unwritten(error: std::io::Error) -> Self {
        Self::new(ErrorCode::RequestFailed, Reason::Unwritten(error))
    }
}

impl Server<'_> {
    /// Asks the client to show the file at `path`.
    fn show(&mut self, path: &Path) -> Result<(), Failure> {
        let path = std::path::absolute(path).map_err(Failure::unwritten)?;
        let uri = Uri::from_file_path(&path)
            .map_err(|e| Failure::new(ErrorCode::InvalidParams, Reason::NotLocal(e)))?;
        let shown = ShowDocumentParams { uri, external: None, take_focus: Some(true), selection: None };
        self.ask::<r::ShowDocument>(&shown);
        Ok(())
    }
}

impl Answers<r::ExecuteCommand> for Server<'_> {
    fn answer(&mut self, params: ExecuteCommandParams) -> Result<Option<Value<'static>>, Failure> {
        let order = Value::from(params.command).read::<Order>().map_err(Failure::params)?;
        let argument = params.arguments.into_iter().flatten().next().unwrap_or_default();
        match order {
            Order::View => self.show_view(&argument.read().map_err(Failure::params)?)?,
            Order::Run => self.run_program(&argument.read().map_err(Failure::params)?)?,
            Order::Input => self.show_input(&argument.read().map_err(Failure::params)?)?,
            Order::Test => self.run_test(&argument.read().map_err(Failure::params)?)?,
            Order::Stop => self.stop_runs(),
        }
        Ok(None)
    }
}

impl Answers<r::CodeLens> for Server<'_> {
    fn answer(&mut self, params: CodeLensParams) -> Result<Vec<CodeLens>, Failure> {
        let mut lenses = self.run_lenses(&params.text_document)?;
        lenses.extend(self.test_lenses(&params.text_document)?);
        Ok(lenses)
    }
}
