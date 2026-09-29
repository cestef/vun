//! What the server offers while code is written: the names that may be written, the signature of
//! the call being written, and the types the code does not write.

use vun_driver::editor::Class;
use vun_grammar::TokenKind;
use vun_lsp_protocol::assist::{
    CompletionItem, CompletionItemKind, InlayHint, InlayHintKind, InlayHintParams, ParameterInformation,
    SignatureHelp, SignatureInformation,
};
use vun_lsp_protocol::base::{MarkupContent, MarkupKind, TextDocumentPositionParams};
use vun_lsp_protocol::requests as r;

use crate::failure::Failure;
use crate::methods::Answers;
use crate::server::Server;
use vun_span::ByteRange;

impl Answers<r::SignatureHelp> for Server<'_> {
    fn answer(&mut self, params: TextDocumentPositionParams) -> Result<Option<SignatureHelp>, Failure> {
        let (file, offset) = self.at(&params)?;
        let Some(signature) = self.session.signature(file, offset) else { return Ok(None) };
        let documentation = (!signature.doc.is_empty())
            .then(|| MarkupContent { kind: MarkupKind::Markdown, value: signature.doc.join("\n") });
        let parameters = signature.params.into_iter().map(|label| ParameterInformation { label }).collect();
        let information =
            SignatureInformation { label: signature.label, documentation, parameters: Some(parameters) };
        Ok(Some(SignatureHelp {
            signatures: vec![information],
            active_signature: Some(0),
            active_parameter: u32::try_from(signature.active).ok(),
        }))
    }
}

impl Answers<r::InlayHint> for Server<'_> {
    fn answer(&mut self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>, Failure> {
        let document = self.document(&params.text_document)?;
        let file = document.file;
        let text = self.session.text(file);
        let end = u32::try_from(text.len()).unwrap_or(u32::MAX);
        let offset = |p| document.lines.offset(text, p, self.encoding).unwrap_or(end);
        let range = ByteRange::new(offset(params.range.start), offset(params.range.end));
        let hints = self.session.type_hints(file, range);
        let document = self.document(&params.text_document)?;
        let text = self.session.text(file);
        // `: T`, as a type is written after a name.
        let colon = TokenKind::Colon.text().unwrap_or_default();
        let shown = hints.into_iter().map(|hint| InlayHint {
            position: document.lines.position(text, hint.at, self.encoding),
            label: format!("{colon} {}", hint.ty),
            kind: Some(InlayHintKind::Type),
        });
        Ok(Some(shown.collect()))
    }
}

impl Answers<r::Completion> for Server<'_> {
    fn answer(&mut self, params: TextDocumentPositionParams) -> Result<Vec<CompletionItem>, Failure> {
        let (file, offset) = self.at(&params)?;
        let completions = self.session.completions(file, offset);
        Ok(completions
            .into_iter()
            .map(|c| CompletionItem { label: c.name.as_str().to_owned(), kind: Some(item_kind(c.class)) })
            .collect())
    }
}

/// The protocol's kind of a name of `class`.
const fn item_kind(class: Class) -> CompletionItemKind {
    match class {
        Class::Namespace => CompletionItemKind::Module,
        Class::Type => CompletionItemKind::Struct,
        Class::Function => CompletionItemKind::Function,
        Class::Parameter | Class::Variable => CompletionItemKind::Variable,
        Class::Property => CompletionItemKind::Field,
        Class::Variant => CompletionItemKind::EnumMember,
        Class::Keyword => CompletionItemKind::Keyword,
        Class::Operator => CompletionItemKind::Operator,
        Class::Comment | Class::String | Class::Number | Class::Label | Class::Attribute => {
            CompletionItemKind::Text
        }
    }
}
