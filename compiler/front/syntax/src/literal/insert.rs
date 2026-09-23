//! Adding a field to a record in its source, keeping the layout around it.

use overt_diag::Edit;
use overt_grammar::TokenKind;
use overt_span::{ByteRange, Span};

use super::print::{INDENT, Item, WIDTH, Writer};
use super::{Decl, Literal, Unwritable};

impl Decl<'_> {
    /// The edit that adds the field `.name = value` to this record, read from `source`: after its
    /// last field, on a line of its own when the record spans lines, and the whole record laid
    /// out again when a one-line record would outgrow [`WIDTH`].
    ///
    /// # Errors
    /// Returns why the field cannot be written, or that this is not a record.
    ///
    /// # Panics
    /// Panics if `source` is not the text the record was read from.
    pub fn insert(&self, source: &str, name: &str, value: &Literal<'_>) -> Result<Edit, Unwritable> {
        let Literal::Record(fields) = &self.value else { return Err(Unwritable::NotRecord) };
        let text = Text(source);
        let (lo, hi) = (self.span.range.lo as usize, self.span.range.hi as usize);
        let line = text.line_start(lo);
        let indent = text.indentation(line);
        let field = Item::Field(name, value);
        let comma = Writer::spelling(TokenKind::Comma);
        let Some(last) = fields.last() else {
            let decl = Decl { name, doc: Vec::new(), value: value.clone(), span: self.span };
            return Ok(self.edit(lo, hi, Literal::Record(vec![decl]).laid_out(indent, lo - line)?));
        };
        let end = last.span.range.hi as usize;
        let close = source[..hi].rfind(Writer::spelling(TokenKind::RBrace)).unwrap_or(end).max(end);
        let between = &source[end..close];
        // After the comma that ends the last field, if it has one.
        let after = between.find(comma).map(|i| end + i + comma.len());
        let mut w = Writer::default();
        if between.contains('\n') {
            let inner = text.indentation(text.line_start(last.span.range.lo as usize));
            let field = field.laid_out(inner, inner.len() + comma.len())?;
            return Ok(match after {
                Some(at) => self.edit(at, at, w.line(inner).text(&field).token(TokenKind::Comma).out.clone()),
                None => self.edit(end, end, w.token(TokenKind::Comma).line(inner).text(&field).out.clone()),
            });
        }
        let flat = field.laid_out(indent, 0)?;
        let line_end = source[hi..].find('\n').map_or(source.len(), |i| hi + i);
        if !flat.contains('\n') && line_end - line + comma.len() + 1 + flat.len() <= WIDTH {
            return Ok(match after {
                Some(at) => self.edit(at, at, w.text(" ").text(&flat).token(TokenKind::Comma).out.clone()),
                None => self.edit(end, end, w.token(TokenKind::Comma).text(" ").text(&flat).out.clone()),
            });
        }
        // Too long for one line: every field on its own, the existing ones as written.
        let inner = format!("{indent}{INDENT}");
        w.token(TokenKind::DotLBrace);
        for decl in fields {
            w.line(&inner).field(decl.name)?;
            w.text(&source[decl.span.range.lo as usize..decl.span.range.hi as usize]).token(TokenKind::Comma);
        }
        let field = field.laid_out(&inner, inner.len() + comma.len())?;
        w.line(&inner).text(&field).token(TokenKind::Comma).line(indent).token(TokenKind::RBrace);
        Ok(self.edit(lo, hi, w.out))
    }

    /// The edit that gives this declaration, read from `source`, the value `value`, laid out
    /// from where its value starts.
    ///
    /// # Errors
    /// Returns why the value cannot be written.
    pub fn set(&self, source: &str, value: &Literal<'_>) -> Result<Edit, Unwritable> {
        let text = Text(source);
        let (lo, hi) = (self.span.range.lo as usize, self.span.range.hi as usize);
        let line = text.line_start(lo);
        Ok(self.edit(lo, hi, value.laid_out(text.indentation(line), lo - line)?))
    }

    /// An edit replacing the bytes `lo..hi` of this declaration's file with `text`.
    fn edit(&self, lo: usize, hi: usize, text: String) -> Edit {
        let offset = |at: usize| u32::try_from(at).expect("source offsets fit in u32");
        let range = ByteRange::new(offset(lo), offset(hi));
        Edit { span: Span { file: self.span.file, range }, text, holes: Vec::new() }
    }
}

/// Source text, by lines.
struct Text<'s>(&'s str);

impl<'s> Text<'s> {
    /// Where the line holding `at` starts.
    fn line_start(&self, at: usize) -> usize {
        self.0[..at].rfind('\n').map_or(0, |i| i + 1)
    }

    /// The leading spaces and tabs of the line starting at `line`.
    fn indentation(&self, line: usize) -> &'s str {
        let rest = &self.0[line..];
        &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
    }
}
