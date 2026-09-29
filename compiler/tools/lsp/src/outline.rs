//! The document's outline, folds and formatting, as the driver computes them, in the protocol's
//! terms.

use vun_driver::editor::{Fold, FoldKind, Symbol, SymbolKind};
use vun_lsp_protocol::base::{TextDocumentParams, TextEdit};
use vun_lsp_protocol::language::{
    self as protocol, DocumentSymbol, FoldingRange, FoldingRangeKind, FormattingParams,
};
use vun_lsp_protocol::requests as r;

use crate::failure::Failure;
use crate::methods::Answers;
use crate::server::{Document, Server};

impl Answers<r::DocumentSymbol> for Server<'_> {
    fn answer(&mut self, params: TextDocumentParams) -> Result<Vec<DocumentSymbol>, Failure> {
        let file = self.document(&params.text_document)?.file;
        let outline = self.session.outline(file);
        let document = self.document(&params.text_document)?;
        Ok(outline.iter().map(|s| self.symbol(document, s)).collect())
    }
}

impl Answers<r::FoldingRange> for Server<'_> {
    fn answer(&mut self, params: TextDocumentParams) -> Result<Vec<FoldingRange>, Failure> {
        let file = self.document(&params.text_document)?.file;
        let folds = self.session.folds(file);
        let document = self.document(&params.text_document)?;
        Ok(folds.iter().filter_map(|f| Self::fold(document, f)).collect())
    }
}

impl Answers<r::Formatting> for Server<'_> {
    fn answer(&mut self, params: FormattingParams) -> Result<Option<Vec<TextEdit>>, Failure> {
        let document = self.document(&params.text_document)?;
        let Some(laid) = self.session.formatted(document.file) else { return Ok(None) };
        let text = self.session.text(document.file);
        Ok(Some(self.edit(document, text, &laid).into_iter().collect()))
    }
}

impl Server<'_> {
    fn symbol(&self, document: &Document, symbol: &Symbol) -> DocumentSymbol {
        let text = self.session.text(document.file);
        let range =
            |span: vun_span::Span| document.lines.range(text, span.range.lo, span.range.hi, self.encoding);
        let children: Vec<DocumentSymbol> =
            symbol.children.iter().map(|c| self.symbol(document, c)).collect();
        DocumentSymbol {
            name: symbol.name.clone(),
            detail: None,
            kind: kind(symbol.kind),
            range: range(symbol.span),
            selection_range: range(symbol.name_span),
            children: (!children.is_empty()).then_some(children),
        }
    }

    /// The lines `fold` spans, when it spans several.
    fn fold(document: &Document, fold: &Fold) -> Option<FoldingRange> {
        let (start, end) = (document.lines.line(fold.range.lo), document.lines.line(fold.range.hi));
        let kind = match fold.kind {
            FoldKind::Region => None,
            FoldKind::Comment => Some(FoldingRangeKind::Comment),
        };
        (start < end).then_some(FoldingRange {
            start_line: start,
            start_character: None,
            end_line: end,
            end_character: None,
            kind,
        })
    }

    /// The one edit making `text` into `laid`: what lies between their common start and end.
    fn edit(&self, document: &Document, text: &str, laid: &str) -> Option<TextEdit> {
        if text == laid {
            return None;
        }
        let differs = text.char_indices().zip(laid.chars()).find(|&((_, a), b)| a != b);
        let prefix = differs.map_or(text.len().min(laid.len()), |((i, _), _)| i);
        let (rest, laid_rest) = (&text[prefix..], &laid[prefix..]);
        let same = rest.chars().rev().zip(laid_rest.chars().rev()).take_while(|(a, b)| a == b);
        let suffix: usize = same.map(|(c, _)| c.len_utf8()).sum();
        let (lo, hi) = (prefix, text.len() - suffix);
        let offset = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Some(TextEdit {
            range: document.lines.range(text, offset(lo), offset(hi), self.encoding),
            new_text: laid[prefix..laid.len() - suffix].to_owned(),
        })
    }
}

/// The protocol's kind of `kind`.
const fn kind(kind: SymbolKind) -> protocol::SymbolKind {
    match kind {
        SymbolKind::Function => protocol::SymbolKind::Function,
        SymbolKind::Struct => protocol::SymbolKind::Struct,
        SymbolKind::Enum => protocol::SymbolKind::Enum,
        SymbolKind::Variant => protocol::SymbolKind::EnumMember,
        SymbolKind::Field => protocol::SymbolKind::Field,
        SymbolKind::Constant => protocol::SymbolKind::Constant,
        SymbolKind::Variable => protocol::SymbolKind::Variable,
    }
}
