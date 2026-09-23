//! Adding a field to a record in its source, keeping the layout around it.

use overt_diag::Edit;
use overt_grammar::TokenKind;
use overt_span::{ByteRange, Span};

use super::print::{INDENT, WIDTH, laid_out_field, token, write_field_prefix};
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
        let range = self.span.range;
        let (lo, hi) = (range.lo as usize, range.hi as usize);
        let line = line_start(source, lo);
        let indent = indentation(source, line);
        let edit = |at: usize, until: usize, text: String| {
            let at = u32::try_from(at).expect("source offsets fit in u32");
            let until = u32::try_from(until).expect("source offsets fit in u32");
            Edit {
                span: Span { file: self.span.file, range: ByteRange::new(at, until) },
                text,
                holes: Vec::new(),
            }
        };
        let comma = token(TokenKind::Comma);
        let Some(last) = fields.last() else {
            let decl = Decl { name, doc: Vec::new(), value: value.clone(), span: self.span };
            let record = Literal::Record(vec![decl]);
            return Ok(edit(lo, hi, record.laid_out(indent, lo - line)?));
        };
        let end = last.span.range.hi as usize;
        let close = source[..hi].rfind(token(TokenKind::RBrace)).unwrap_or(end).max(end);
        let between = &source[end..close];
        let after = between.find(comma).map(|i| end + i + comma.len());
        if between.contains('\n') {
            let inner = indentation(source, line_start(source, last.span.range.lo as usize));
            let field = laid_out_field(name, value, inner, inner.len() + comma.len())?;
            return Ok(match after {
                Some(at) => edit(at, at, format!("\n{inner}{field}{comma}")),
                None => edit(end, end, format!("{comma}\n{inner}{field}")),
            });
        }
        let field = laid_out_field(name, value, indent, 0)?;
        let line_end = source[hi..].find('\n').map_or(source.len(), |i| hi + i);
        if !field.contains('\n') && line_end - line + comma.len() + 1 + field.len() <= WIDTH {
            return Ok(match after {
                Some(at) => edit(at, at, format!(" {field}{comma}")),
                None => edit(end, end, format!("{comma} {field}")),
            });
        }
        // Too long for one line: every field on its own, the existing ones as written.
        let inner = format!("{indent}{INDENT}");
        let mut out = token(TokenKind::DotLBrace).to_owned();
        for decl in fields {
            write_field_prefix(decl.name, out.push_line(&inner))?;
            out.push_str(&source[decl.span.range.lo as usize..decl.span.range.hi as usize]);
            out.push_str(comma);
        }
        out.push_line(&inner);
        out.push_str(&laid_out_field(name, value, &inner, inner.len() + comma.len())?);
        out.push_str(comma);
        out.push_line(indent);
        out.push_str(token(TokenKind::RBrace));
        Ok(edit(lo, hi, out))
    }
}

/// Appending a line break and an indentation.
trait PushLine {
    fn push_line(&mut self, indent: &str) -> &mut Self;
}

impl PushLine for String {
    fn push_line(&mut self, indent: &str) -> &mut Self {
        self.push('\n');
        self.push_str(indent);
        self
    }
}

fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// The leading spaces and tabs of the line starting at `line`.
fn indentation(source: &str, line: usize) -> &str {
    let rest = &source[line..];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}
