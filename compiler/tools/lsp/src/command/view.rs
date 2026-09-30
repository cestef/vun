//! Views of a document: each representation `vun debug dump` prints, written beside it.

use vun_data::{FromData, ToData};
use vun_driver::view::View;
use vun_fs::Edit;
use vun_lsp_protocol::base::TextDocumentIdentifier;
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
}

impl Server<'_> {
    /// The views of the document `text_document`, each a code action showing it.
    pub(crate) fn view_actions(
        &mut self,
        text_document: &TextDocumentIdentifier,
    ) -> Result<Vec<CodeAction>, Failure> {
        let file = self.document(text_document)?.file;
        let actions = self.session.views(file).into_iter().map(|view| {
            let title = ["Show ", view.name()].concat();
            let viewed = Viewed { text_document: text_document.clone(), view };
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
    pub(super) fn show_view(&mut self, viewed: &Viewed) -> Result<(), Failure> {
        let document = self.document(&viewed.text_document)?;
        let (file, source) = (document.file, document.path.clone());
        let path = self.session.shown_at(&source, viewed.view.name());
        let mut text = Vec::new();
        let written = self.session.view(viewed.view, file, &mut text).and_then(|()| path.put(text));
        written.map_err(Failure::unwritten)?;
        // What computing it reports is the document's, which publishing shows.
        drop(std::mem::take(&mut self.session.diag));
        self.show(&path)
    }
}
