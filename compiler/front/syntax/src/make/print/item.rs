//! Files, declarations and what a struct or an enum holds.

use vun_grammar::TokenKind;

use super::Place;
use crate::lexer::{DOC_COMMENT, MODULE_DOC_COMMENT};
use crate::literal::Writer;
use crate::make::{Bind, Expr, Field, File, Item, Member, Variant};

impl Writer {
    pub(super) fn file(&mut self, file: &File) {
        for line in &file.doc {
            self.text(MODULE_DOC_COMMENT).text(" ").text(line).line("");
        }
        for (i, path) in file.spreads.iter().enumerate() {
            if i == 0 && !self.out.is_empty() {
                self.line("");
            }
            self.token(TokenKind::DotDot);
            self.expr(&Expr::Path(path.clone()), Place::TOP);
            self.token(TokenKind::Semi).line("");
        }
        for item in &file.items {
            if !self.out.is_empty() {
                self.line("");
            }
            self.item(item, Place::TOP);
            self.token(TokenKind::Semi).line("");
        }
    }

    /// The docs, attributes and binding of `item`, without the closing `;`.
    pub(super) fn item(&mut self, item: &Item, at: Place) {
        for line in &item.doc {
            self.text(DOC_COMMENT).text(" ").text(line).newline(at);
        }
        if !item.attrs.is_empty() {
            self.attributes(&item.attrs, at).newline(at);
        }
        self.bind(&item.bind, at);
    }

    /// `#[a, b]`
    pub(super) fn attributes(&mut self, attrs: &[Expr], at: Place) -> &mut Self {
        self.token(TokenKind::HashLBracket).list(attrs, at);
        self.token(TokenKind::RBracket)
    }

    /// `mut pattern: ty = value`, `pattern := value`, or `pattern: ty` uninitialized.
    pub(super) fn bind(&mut self, bind: &Bind, at: Place) {
        if bind.mutable {
            self.token(TokenKind::Mut).text(" ");
        }
        self.operand(&bind.pattern, Expr::value_level(), at.within());
        match (&bind.ty, &bind.value) {
            (Some(ty), value) => {
                self.token(TokenKind::Colon).text(" ");
                self.operand(ty, Expr::value_level(), at.within());
                if let Some(value) = value {
                    self.text(" ").token(TokenKind::Eq).text(" ");
                    self.operand(value, Expr::value_level(), at);
                }
            }
            (None, value) => {
                self.text(" ").token(TokenKind::ColonEq).text(" ");
                // A binding without a type or a value says nothing; the hole is what it can mean.
                self.operand(value.as_ref().unwrap_or(&Expr::Hole), Expr::value_level(), at);
            }
        }
    }

    /// `name: ty = default`
    pub(super) fn struct_field(&mut self, field: &Field, at: Place) {
        self.binder(&field.name).expect("made names are identifiers");
        self.token(TokenKind::Colon).text(" ");
        self.operand(&field.ty, Expr::value_level(), at.within());
        if let Some(default) = &field.default {
            self.text(" ").token(TokenKind::Eq).text(" ");
            self.operand(default, Expr::value_level(), at.within());
        }
    }

    /// Each member on a line of its own: fields end with `,`, declarations with `;`.
    pub(super) fn members(&mut self, members: &[Member], at: Place) {
        let inner = at.deeper();
        for member in members {
            self.newline(inner);
            match member {
                Member::Field(field) => {
                    self.struct_field(field, inner);
                    self.token(TokenKind::Comma);
                }
                Member::Item(item) => {
                    self.item(item, inner);
                    self.token(TokenKind::Semi);
                }
            }
        }
    }

    /// `struct { members }`
    pub(super) fn struct_type(&mut self, members: &[Member], at: Place) {
        self.token(TokenKind::Struct).text(" ").token(TokenKind::LBrace);
        self.members(members, at);
        if !members.is_empty() {
            self.newline(at);
        }
        self.token(TokenKind::RBrace);
    }

    /// `enum { a, b(T); members }`
    pub(super) fn enum_type(&mut self, variants: &[Variant], members: &[Member], at: Place) {
        self.token(TokenKind::Enum).text(" ").token(TokenKind::LBrace);
        let inner = at.deeper();
        if variants.is_empty() && !members.is_empty() {
            self.newline(inner).token(TokenKind::Semi);
        }
        for (i, variant) in variants.iter().enumerate() {
            self.newline(inner);
            self.binder(&variant.name).expect("made names are identifiers");
            if let Some(code) = &variant.code {
                self.text(" ").token(TokenKind::Eq).text(" ");
                self.expr(code, inner);
            }
            if !variant.payload.is_empty() {
                self.token(TokenKind::LParen).list(&variant.payload, inner);
                self.token(TokenKind::RParen);
            }
            if i + 1 < variants.len() || members.is_empty() {
                self.token(TokenKind::Comma);
            } else {
                self.token(TokenKind::Semi);
            }
        }
        self.members(members, at);
        if !variants.is_empty() || !members.is_empty() {
            self.newline(at);
        }
        self.token(TokenKind::RBrace);
    }
}
