//! Semantic tokens: the driver's highlights numbered by the legend and encoded as the protocol
//! encodes them, each relative to the one before.

use vun_driver::editor::{Class, Modifier};
use vun_lsp_protocol::base::TextDocumentParams;
use vun_lsp_protocol::language::{SemanticTokens, TokenModifier, TokenType};
use vun_lsp_protocol::requests as r;

use crate::failure::Failure;
use crate::methods::Answers;
use crate::server::Server;

impl Answers<r::SemanticTokens> for Server<'_> {
    fn answer(&mut self, params: TextDocumentParams) -> Result<SemanticTokens, Failure> {
        let file = self.document(&params.text_document)?.file;
        let highlights = self.session.highlights(file);
        let document = self.document(&params.text_document)?;
        let text = self.session.text(file);
        let mut data = Vec::with_capacity(highlights.len() * 5);
        let (mut line, mut start) = (0, 0);
        for h in highlights {
            let range = document.lines.range(text, h.range.lo, h.range.hi, self.encoding);
            // A token is on one line; one that is not, a comment ending in its line break, is cut there.
            let length = if range.end.line == range.start.line {
                range.end.character - range.start.character
            } else {
                let end =
                    document.lines.position(text, h.range.hi.min(line_end(text, h.range.lo)), self.encoding);
                end.character - range.start.character
            };
            let delta_line = range.start.line - line;
            let delta_start =
                if delta_line == 0 { range.start.character - start } else { range.start.character };
            let modifiers = h.modifiers.iter().map(|m| modifier(m).bit()).fold(0, |a, b| a | b);
            data.extend([delta_line, delta_start, length, token_type(h.class).number(), modifiers]);
            (line, start) = (range.start.line, range.start.character);
        }
        Ok(SemanticTokens { result_id: None, data })
    }
}

/// Where the line holding the byte `at` of `text` ends, before its line break.
fn line_end(text: &str, at: u32) -> u32 {
    let at = at as usize;
    let end = text[at..].find('\n').map_or(text.len(), |p| at + p);
    u32::try_from(end).unwrap_or(u32::MAX)
}

/// The protocol's kind of `class`.
const fn token_type(class: Class) -> TokenType {
    match class {
        Class::Namespace => TokenType::Namespace,
        Class::Type => TokenType::Type,
        Class::Function => TokenType::Function,
        Class::Variable => TokenType::Variable,
        Class::Parameter => TokenType::Parameter,
        Class::Property => TokenType::Property,
        Class::Variant => TokenType::EnumMember,
        Class::Keyword => TokenType::Keyword,
        Class::Comment => TokenType::Comment,
        Class::String => TokenType::String,
        Class::Number => TokenType::Number,
        Class::Operator => TokenType::Operator,
        Class::Label => TokenType::Label,
        Class::Attribute => TokenType::Decorator,
    }
}

/// The protocol's modifier of `modifier`.
const fn modifier(modifier: Modifier) -> TokenModifier {
    match modifier {
        Modifier::Declaration => TokenModifier::Declaration,
        Modifier::Mutable => TokenModifier::Mutable,
        Modifier::Readonly => TokenModifier::Readonly,
        Modifier::Comptime => TokenModifier::Comptime,
        Modifier::Documentation => TokenModifier::Documentation,
    }
}
