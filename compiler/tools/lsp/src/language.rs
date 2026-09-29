//! What the server tells of a position: the type of the expression there, and where the name
//! there is declared.

use std::path::Path;

use vun_driver::editor::Target;
use vun_lsp_protocol::base::{Location, MarkupContent, MarkupKind, Range, TextDocumentPositionParams, Uri};
use vun_lsp_protocol::language::Hover;
use vun_lsp_protocol::requests as r;
use vun_span::FileId;

use crate::failure::Failure;
use crate::methods::Answers;
use crate::server::Server;

/// The fence the type of a hover is shown in, as Markdown, and the language it names.
const FENCE: &str = "```";
const LANGUAGE: &str = "vun";

impl Server<'_> {
    /// The file and byte offset `params` name.
    pub(crate) fn at(&self, params: &TextDocumentPositionParams) -> Result<(FileId, u32), Failure> {
        let document = self.document(&params.text_document)?;
        let text = self.session.text(document.file);
        let offset = document.lines.offset(text, params.position, self.encoding);
        Ok((document.file, offset.unwrap_or(u32::try_from(text.len()).unwrap_or(u32::MAX))))
    }

    /// The location of the start of the file at `path`.
    fn start_of(path: &Path) -> Option<Location> {
        let path = std::path::absolute(path).ok()?;
        Some(Location { uri: Uri::from_file_path(&path).ok()?, range: Range::default() })
    }
}

impl Answers<r::Hover> for Server<'_> {
    fn answer(&mut self, params: TextDocumentPositionParams) -> Result<Option<Hover>, Failure> {
        let (file, offset) = self.at(&params)?;
        let Some(hovered) = self.session.hovered(file, offset) else { return Ok(None) };
        let mut shown = Vec::new();
        if let Some(known) = hovered.known {
            shown.push(format!("{FENCE}{LANGUAGE}\n{}\n{FENCE}\n{}", known.ty, known.stage.name()));
        }
        if !hovered.doc.is_empty() {
            shown.push(hovered.doc.join("\n"));
        }
        if shown.is_empty() {
            return Ok(None);
        }
        let document = self.document(&params.text_document)?;
        let text = self.session.text(file);
        let range = document.lines.range(text, hovered.span.range.lo, hovered.span.range.hi, self.encoding);
        let contents = MarkupContent { kind: MarkupKind::Markdown, value: shown.join("\n\n") };
        Ok(Some(Hover { contents, range: Some(range) }))
    }
}

impl Answers<r::Definition> for Server<'_> {
    fn answer(&mut self, params: TextDocumentPositionParams) -> Result<Option<Location>, Failure> {
        let (file, offset) = self.at(&params)?;
        Ok(match self.session.declared(file, offset) {
            Some(Target::At(span)) => self.location(span),
            Some(Target::File(path)) => Self::start_of(&path),
            None => None,
        })
    }
}
