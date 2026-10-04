//! Views of a document: each representation `vun debug dump` prints, written beside it.

use vun_data::{FromData, ToData};
use vun_driver::view::{Scope, Unit, View};
use vun_fs::Edit;
use vun_lsp_protocol::base::{Position, TextDocumentIdentifier};
use vun_lsp_protocol::edit::{CodeAction, CodeActionKind};

use super::Order;
use crate::failure::Failure;
use crate::server::Server;

/// The arguments of [`Order::View`].
#[derive(Clone, Debug, PartialEq, Eq, ToData, FromData)]
#[data(case = camel)]
pub(crate) struct Viewed {
    pub(crate) text_document: TextDocumentIdentifier,
    pub(crate) view: View,
    pub(crate) scope: Scope,
}

impl Viewed {
    /// What the action showing it says: `Show types`, or `Show types of this function`.
    fn title(&self) -> String {
        match (self.scope, self.view.unit()) {
            (Scope::At { .. }, Some(unit)) => ["Show ", self.view.name(), " of this ", unit.name()].concat(),
            _ => ["Show ", self.view.name()].concat(),
        }
    }
}

impl Server<'_> {
    /// The views of the document `text_document`, each a code action showing it, and those that
    /// narrow, again for the function holding `at`, if one does.
    pub(crate) fn view_actions(
        &mut self,
        text_document: &TextDocumentIdentifier,
        at: Position,
    ) -> Result<Vec<CodeAction>, Failure> {
        let document = self.document(text_document)?;
        let file = document.file;
        let offset = document.lines.offset(self.session.text(file), at, self.encoding);
        let views = self.session.views(file);
        let whole = views.iter().map(|&view| (view, Scope::File));
        let mut narrowed = Vec::new();
        if let Some(offset) = offset {
            for &view in &views {
                let held = view.unit().is_some_and(|unit| self.session.has_unit(unit, file, offset));
                if held {
                    narrowed.push((view, Scope::At { offset }));
                }
            }
        }
        let actions = whole.chain(narrowed).map(|(view, scope)| {
            let viewed = Viewed { text_document: text_document.clone(), view, scope };
            let title = viewed.title();
            CodeAction {
                title: title.clone(),
                kind: Some(CodeActionKind::Source),
                diagnostics: None,
                is_preferred: None,
                edit: None,
                command: Some(Order::View.command(title, &viewed)),
            }
        });
        Ok(actions.collect())
    }

    /// Writes the view `viewed` asks for beside its document, and asks the client to show it.
    pub(crate) fn show_view(&mut self, viewed: &Viewed) -> Result<(), Failure> {
        let document = self.document(&viewed.text_document)?;
        let (file, source) = (document.file, document.path.clone());
        let unit = viewed.view.unit().filter(|_| viewed.scope != Scope::File).map(Unit::name);
        let shown = [Some(viewed.view.name()), unit].into_iter().flatten();
        let path = self.session.shown_at(&source, &shown.collect::<Vec<_>>().join("."));
        let mut text = Vec::new();
        let written =
            self.session.view(viewed.view, file, viewed.scope, &mut text).and_then(|()| path.put(text));
        written.map_err(Failure::unwritten)?;
        // What computing it reports is the document's, which publishing shows.
        drop(std::mem::take(&mut self.session.diag));
        self.show(&path)
    }
}
