//! Writing made source: items one after the other with a blank line between, attributes on the
//! line before their item, a struct type's fields one per line.

use vun_grammar::TokenKind;

use super::{Expr, Field, File, Item};
use crate::lexer::{DOC_COMMENT, MODULE_DOC_COMMENT};
use crate::literal::{INDENT, Literal, Writer};

pub(super) fn file(file: &File) -> String {
    let mut w = Writer::default();
    for line in &file.doc {
        w.text(MODULE_DOC_COMMENT).text(" ").text(line).line("");
    }
    for item in &file.items {
        if !w.out.is_empty() {
            w.line("");
        }
        w.item(item);
    }
    w.out
}

impl Writer {
    fn item(&mut self, item: &Item) {
        for line in &item.doc {
            self.text(DOC_COMMENT).text(" ").text(line).line("");
        }
        if !item.attrs.is_empty() {
            self.attributes(&item.attrs, "").line("");
        }
        if item.mutable {
            self.token(TokenKind::Mut).text(" ");
        }
        self.made_name(&item.name);
        match &item.ty {
            Some(ty) => {
                self.token(TokenKind::Colon).text(" ");
                self.expr(ty, "");
                self.text(" ").token(TokenKind::Eq).text(" ");
            }
            None => {
                self.text(" ").token(TokenKind::ColonEq).text(" ");
            }
        }
        self.expr(&item.value, "");
        self.token(TokenKind::Semi).line("");
    }

    /// `#[a, b]`
    fn attributes(&mut self, attrs: &[Expr], indent: &str) -> &mut Self {
        self.token(TokenKind::HashLBracket);
        for (i, attr) in attrs.iter().enumerate() {
            if i > 0 {
                self.token(TokenKind::Comma).text(" ");
            }
            self.expr(attr, indent);
        }
        self.token(TokenKind::RBracket)
    }

    /// A name, escaped when it is a keyword; a name no escape makes one is a bug of the maker.
    fn made_name(&mut self, name: &str) -> &mut Self {
        self.binder(name).expect("made names are identifiers")
    }

    /// `expr`, whose lines after the first are indented by `indent`.
    fn expr(&mut self, expr: &Expr, indent: &str) {
        match expr {
            Expr::Int(n) => self.literal(&Literal::Int(*n)),
            Expr::String(s) => self.string(s),
            Expr::Hole => {
                self.token(TokenKind::Underscore);
            }
            Expr::Unit => {
                self.token(TokenKind::LParen).token(TokenKind::RParen);
            }
            Expr::Path(path) => {
                for (i, name) in path.iter().enumerate() {
                    if i > 0 {
                        self.token(TokenKind::Dot);
                    }
                    self.made_name(name);
                }
            }
            Expr::Call(callee, args) => {
                self.expr(callee, indent);
                self.token(TokenKind::LParen);
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.token(TokenKind::Comma).text(" ");
                    }
                    self.expr(arg, indent);
                }
                self.token(TokenKind::RParen);
            }
            Expr::Prefix(token, inner) => {
                self.token(*token);
                // A keyword prefix, such as `unsafe`, is a word apart from what follows.
                if TokenKind::KEYWORDS.contains(token) {
                    self.text(" ");
                }
                self.expr(inner, indent);
            }
            Expr::Array(len, item) => {
                self.token(TokenKind::LBracket);
                self.expr(len, indent);
                self.token(TokenKind::RBracket);
                self.expr(item, indent);
            }
            Expr::Attributed(attrs, inner) => {
                self.attributes(attrs, indent).text(" ");
                self.expr(inner, indent);
            }
            Expr::Fn(params, result) => {
                self.token(TokenKind::LParen);
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.token(TokenKind::Comma).text(" ");
                    }
                    self.typed_field(param, indent);
                }
                self.token(TokenKind::RParen);
                if let Some(result) = result {
                    self.text(" ").token(TokenKind::Arrow).text(" ");
                    self.expr(result, indent);
                }
            }
            Expr::Struct(fields) => {
                self.token(TokenKind::Struct).text(" ").token(TokenKind::LBrace);
                let inner = [indent, INDENT].concat();
                for field in fields {
                    self.line(&inner);
                    self.typed_field(field, &inner);
                    self.token(TokenKind::Comma);
                }
                if !fields.is_empty() {
                    self.line(indent);
                }
                self.token(TokenKind::RBrace);
            }
        }
    }

    /// `name: ty`
    fn typed_field(&mut self, field: &Field, indent: &str) {
        self.made_name(&field.name).token(TokenKind::Colon).text(" ");
        self.expr(&field.ty, indent);
    }

    fn literal(&mut self, literal: &Literal<'_>) {
        self.value(literal).expect("integers are literals");
    }
}
