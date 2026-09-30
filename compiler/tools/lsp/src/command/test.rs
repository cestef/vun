//! Running a test: a lens over each test a document declares, and the command building the
//! tests' program of the document's namespace and running that test, what it reports written
//! beside the document as it comes.

use vun_data::{FromData, ToData};
use vun_driver::view::View;
use vun_lsp_protocol::base::TextDocumentIdentifier;
use vun_lsp_protocol::command::CodeLens;
use vun_term::Shell;
use vun_trace::tracing;

use super::Order;
use crate::failure::Failure;
use crate::server::Server;

/// What the report of a test is named by, beside the document.
const OUTPUT: &str = "test";
const TITLE: &str = "Test";

/// The arguments of [`Order::Test`]: the test, by its name, and the document declaring it.
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub(crate) struct Tested {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) name: String,
}

impl Server<'_> {
    /// A lens over each test `document` declares, as the runner finds them.
    pub(super) fn test_lenses(
        &mut self,
        document: &TextDocumentIdentifier,
    ) -> Result<Vec<CodeLens>, Failure> {
        let file = self.document(document)?.file;
        let lenses = self.session.tests(file).into_iter().filter_map(|(name, span)| {
            let range = self.location(span)?.range;
            let tested = Tested { text_document: document.clone(), name: name.as_str().to_owned() };
            Some(CodeLens { range, command: Some(Order::Test.command(TITLE.to_owned(), &tested)) })
        });
        Ok(lenses.collect())
    }

    /// Builds the tests' program of the namespace of the document `tested` names and runs that
    /// test, what it reports written beside the document as it comes, and shown; what stops the
    /// build is written there instead.
    pub(super) fn run_test(&mut self, tested: &Tested) -> Result<(), Failure> {
        let source = self.document(&tested.text_document)?.path.clone();
        let path = View::shown(&source, OUTPUT);
        let mut output = Self::output(&path)?;
        let root = self.session.tests_root(Some(&source)).map_err(Failure::unwritten)?;
        let program = root.and_then(|root| self.session.load(&root)).and_then(|root| {
            let ns = self.session.tested(root, Some(&source))?;
            self.session.build_tests(root, ns, None).ok()
        });
        let Some(program) = program else {
            self.stopped(&mut output)?;
            return self.show(&path);
        };
        drop(std::mem::take(&mut self.session.diag));
        let run = self.session.run_tests(&program, [&tested.name]).map_err(Failure::unwritten)?;
        let mut errors = output.try_clone().map_err(Failure::unwritten)?;
        std::thread::spawn(move || {
            if let Err(error) = run.report(&mut Shell::plain(&mut output, &mut errors)) {
                tracing::warn!(%error, "a test run did not end cleanly");
            }
        });
        self.show(&path)
    }
}
