//! Laying written values out on lines: within the width, or one item per line.

use overt_grammar::TokenKind;

use super::{INDENT, Unwritable, WIDTH, Writer};
use crate::literal::{DOC, Decl, Literal};

impl Decl<'_> {
    /// The declaration `name := value;`, its value laid out within [`WIDTH`].
    ///
    /// # Errors
    /// Returns why the value or the name has no source.
    pub fn source(&self) -> Result<String, Unwritable> {
        self.written(|value, column| value.laid_out("", column))
    }

    /// The declaration `name := value;` with its list or record value holding one item per line,
    /// each on one line whatever its width, so that changes to different items never touch the
    /// same line.
    ///
    /// # Errors
    /// Returns why the value or the name has no source.
    pub fn listed(&self) -> Result<String, Unwritable> {
        self.written(|value, _| {
            let items: Vec<String> = match value {
                Literal::List(items) => items.iter().map(Literal::source).collect::<Result<_, _>>()?,
                Literal::Record(fields) => fields
                    .iter()
                    .map(|d| {
                        let mut w = Writer::default();
                        w.field(d.name)?.value(&d.value)?;
                        Ok(w.out)
                    })
                    .collect::<Result<_, _>>()?,
                _ => return value.source(),
            };
            if items.is_empty() {
                return value.source();
            }
            let mut w = Writer::default();
            w.token(TokenKind::DotLBrace);
            for item in items {
                w.line(INDENT).text(&item).token(TokenKind::Comma);
            }
            w.line("").token(TokenKind::RBrace);
            Ok(w.out)
        })
    }

    /// Its doc comment, then `name := value;` with the value written by `value` from the column
    /// it starts at.
    fn written(
        &self,
        value: impl FnOnce(&Literal<'_>, usize) -> Result<String, Unwritable>,
    ) -> Result<String, Unwritable> {
        let mut w = Writer::default();
        for line in &self.doc {
            w.text(DOC).text(line).text("\n");
        }
        let start = w.out.len();
        w.binder(self.name)?.text(" ").token(TokenKind::ColonEq).text(" ");
        let value = value(&self.value, w.out.len() - start)?;
        w.text(&value).token(TokenKind::Semi).text("\n");
        Ok(w.out)
    }
}

/// What is laid out on a line of its own: a value, or a record's field.
pub(in crate::literal) enum Item<'a, 's> {
    Value(&'a Literal<'s>),
    Field(&'a str, &'a Literal<'s>),
}

impl Item<'_, '_> {
    /// Its source starting `column` characters into a line indented by `indent`, on one line when
    /// it fits within [`WIDTH`].
    pub(in crate::literal) fn laid_out(&self, indent: &str, column: usize) -> Result<String, Unwritable> {
        let value = match *self {
            Self::Field(name, value) => {
                let mut w = Writer::default();
                w.field(name)?;
                let value = Item::Value(value).laid_out(indent, column + w.out.len())?;
                return Ok(w.text(&value).out.clone());
            }
            Self::Value(value) => value,
        };
        let flat = value.source()?;
        let items: Vec<Item<'_, '_>> = match value {
            Literal::List(items) => items.iter().map(Item::Value).collect(),
            Literal::Record(fields) => fields.iter().map(|d| Item::Field(d.name, &d.value)).collect(),
            _ => Vec::new(),
        };
        if items.is_empty() || column + flat.len() <= WIDTH {
            return Ok(flat);
        }
        let inner = format!("{indent}{INDENT}");
        // The comma after each item counts toward the width.
        let column = inner.len() + Writer::spelling(TokenKind::Comma).len();
        let mut w = Writer::default();
        w.token(TokenKind::DotLBrace);
        for item in items {
            let item = item.laid_out(&inner, column)?;
            w.line(&inner).text(&item).token(TokenKind::Comma);
        }
        w.line(indent).token(TokenKind::RBrace);
        Ok(w.out)
    }
}
