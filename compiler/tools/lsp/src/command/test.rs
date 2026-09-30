//! Running a test: a lens over each test a document declares, and the command building the
//! tests' program of the document's namespace and running that test, what it reports written
//! beside the document as it comes.

use vun_data::{FromData, ToData};
use vun_driver::testing::{Outcome, Place, Ran, Said, Trapped};
use vun_driver::view::View;
use vun_lsp_protocol::base::{Position, PositionEncoding, TextDocumentIdentifier, Uri};
use vun_lsp_protocol::command::CodeLens;
use vun_lsp_protocol::document::{Diagnostic, Severity};
use vun_lsp_protocol::requests as r;
use vun_term::Shell;
use vun_trace::tracing;

use super::Order;
use crate::documents::SOURCE;
use crate::failure::Failure;
use crate::server::{Document, Event, Server};

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
        let results = self.results.get(&document.uri);
        let lenses = self.session.tests(file).into_iter().filter_map(|(name, span)| {
            let range = self.location(span)?.range;
            let name = name.as_str().to_owned();
            let title = match results.and_then(|r| r.get(&name)) {
                None => TITLE.to_owned(),
                Some(outcome) => format!("{TITLE}: {}", Self::said(outcome)),
            };
            let tested = Tested { text_document: document.clone(), name };
            Some(CodeLens { range, command: Some(Order::Test.command(title, &tested)) })
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
        let (events, uri) = (self.events.clone(), tested.text_document.uri.clone());
        std::thread::spawn(move || match run.report(&mut Shell::plain(&mut output, &mut errors)) {
            // The server may have ended since.
            Ok(reported) => drop(events.send(Event::Tested { uri, ran: reported.ran })),
            Err(error) => tracing::warn!(%error, "a test run did not end cleanly"),
        });
        self.show(&path)
    }

    /// Keeps what a run of the tests of the document at `uri` reported, until the document
    /// changes: the titles of its lenses and the traps shown where they fired change with it.
    pub(crate) fn tested(&mut self, uri: Uri, ran: Vec<Ran>) {
        let results = self.results.entry(uri).or_default();
        results.extend(ran.into_iter().map(|ran| (ran.name, ran.outcome)));
        self.changed.extend(self.documents.keys().cloned());
        self.ask::<r::CodeLensRefresh>(&None);
    }

    /// Each trap a test's last run fired in `document`, where it fired.
    pub(crate) fn traps(&self, document: &Document) -> Vec<Diagnostic> {
        let tests = self.results.values().flatten();
        let traps = tests.filter_map(|(name, outcome)| match outcome {
            Outcome::Trapped(Trapped { said: Some(Said { message, at: Some(at) }), .. }) => {
                Some((name, message, at))
            }
            _ => None,
        });
        traps
            .filter(|(_, _, at)| std::path::absolute(&at.file).is_ok_and(|file| file == document.path))
            .filter_map(|(name, message, at)| {
                let position = self.position(document, at)?;
                let message = match message {
                    Some(message) => format!("the test `{name}` trapped: {message}"),
                    None => format!("the test `{name}` trapped"),
                };
                Some(Diagnostic {
                    range: vun_lsp_protocol::base::Range { start: position, end: position },
                    severity: Some(Severity::Error),
                    code: None,
                    code_description: None,
                    source: Some(SOURCE.to_owned()),
                    message,
                    related_information: None,
                })
            })
            .collect()
    }

    /// Where `at`, a line and a column of characters counted from 1, is in `document`.
    fn position(&self, document: &Document, at: &Place) -> Option<Position> {
        let text = self.session.text(document.file);
        let place = Position { line: at.line.checked_sub(1)?, character: at.column.checked_sub(1)? };
        let offset = document.lines.offset(text, place, PositionEncoding::Utf32)?;
        Some(document.lines.position(text, offset, self.encoding))
    }

    /// What became of a test, as a lens's title says it.
    const fn said(outcome: &Outcome) -> &'static str {
        match outcome {
            Outcome::Passed => "passed",
            Outcome::Ignored(_) => "ignored",
            Outcome::Trapped(_) => "trapped",
        }
    }
}
