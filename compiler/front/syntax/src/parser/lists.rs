//! Lists and their items, delimited groups and annotations.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{NodeKind, TokenKind};

use super::{Brace, Form, Item, List, Parser, Shape};
use crate::cst::Anchor;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    /// Entries up to `close`, which is left for the caller.
    pub(super) fn list(&mut self, list: List, close: TokenKind) -> Shape {
        let mut shape = Shape::default();
        let start = self.out.mark();
        while !self.at(close) && !self.at(TokenKind::Eof) {
            let before = self.pos;
            let proceed = if let List::Items(item) = list {
                self.item(item, close, &mut shape)
            } else {
                self.entry(list, close);
                true
            };
            if self.pos == before && !self.at(close) && !self.at(TokenKind::Eof) {
                self.report_expected_entry(list);
                self.leaf(NodeKind::Error);
            }
            if !proceed {
                break;
            }
        }
        // Counted from the tree, so that skipped tokens count as items and a list holding junk is
        // never taken for a single parenthesized or bracketed operand.
        shape.items = self.out.nodes_since(start, |k| k != NodeKind::Punct);
        shape
    }

    /// A braced list, the `{` being a punctuation leaf of the enclosing node.
    pub(super) fn braced(&mut self, list: List) {
        let open = self.current();
        if self.eat(TokenKind::LBrace) {
            self.list(list, TokenKind::RBrace);
            self.close(TokenKind::RBrace, open);
        } else {
            self.report_expected(&[TokenKind::LBrace]);
            self.missing(NodeKind::Punct);
        }
    }

    /// `{ variants }` or `{ variants; declarations }`: an enum's variants, then, after `;`, members
    /// as in a struct body, its namespace.
    pub(super) fn enum_body(&mut self) {
        let open = self.current();
        if !self.eat(TokenKind::LBrace) {
            self.report_expected(&[TokenKind::LBrace]);
            self.missing(NodeKind::Punct);
            return;
        }
        self.list(List::Items(Item::Value), TokenKind::RBrace);
        // The declarations after `;` are the enum's namespace, apart from its variants.
        if self.at(TokenKind::Semi) {
            let mark = self.out.mark();
            let semi = self.bump();
            self.list(List::Members, TokenKind::RBrace);
            self.out.node(NodeKind::Namespace, Anchor::Token(semi), mark);
        }
        self.close(TokenKind::RBrace, open);
    }

    /// `{ list }` as a `Block` node.
    pub(super) fn block(&mut self, list: List) {
        if !self.at(TokenKind::LBrace) {
            self.report_expected(&[TokenKind::LBrace]);
            self.missing(NodeKind::Error);
            return;
        }
        self.nested(|p| {
            let mark = p.out.mark();
            let open = p.bump();
            p.list(list, TokenKind::RBrace);
            p.close(TokenKind::RBrace, open);
            p.out.node(NodeKind::Block, Anchor::Token(open), mark);
        });
    }

    /// A delimited list whose opening token anchors the node the caller pushes.
    pub(super) fn group_of(&mut self, list: List) -> (TokenId, Shape) {
        let open = self.bump();
        let close = self.kind(open).closer().expect("group_of starts at an opening delimiter");
        let shape = self.list(list, close);
        self.close(close, open);
        (open, shape)
    }

    /// The closing token of a group opened by `open`.
    pub(super) fn close(&mut self, close: TokenKind, open: TokenId) {
        if self.eat(close) {
            return;
        }
        let opener = self.text(open).to_owned();
        let msg = Msg::new().text("unclosed ").code(opener);
        let help = Msg::new().text("expected ").code(close.text().unwrap_or_default()).text(", found ");
        let closer = close.text().unwrap_or_default();
        let d = Diagnostic::error(errors::SYNTAX_UNCLOSED_DELIMITER, msg)
            .child(ChildKind::Help, self.found(help))
            .fix(self.insertion(closer));
        self.report(self.tokens.span(open), d);
        self.missing(NodeKind::Punct);
    }

    /// One item and its separator; `false` ends the list.
    pub(super) fn item(&mut self, item: Item, close: TokenKind, shape: &mut Shape) -> bool {
        if self.at(TokenKind::Dollar) && self.block_like_at() {
            self.block_like(List::Items(item));
        } else {
            self.annotated(|p| p.item_body(item, shape));
        }
        shape.items += 1;
        if self.eat(TokenKind::Comma) {
            shape.commas += 1;
            return true;
        }
        if item == Item::Element && shape.single() && self.eat(TokenKind::Semi) {
            self.expr(Brace::Continues);
            shape.form = Form::Repeated;
            return false;
        }
        let k = self.peek();
        if k == close || k == TokenKind::Semi || k.closes() || k == TokenKind::Eof {
            return false;
        }
        self.report_expected(&[TokenKind::Comma, close]);
        self.recover(&[TokenKind::Comma, TokenKind::Semi]);
        if self.eat(TokenKind::Comma) {
            shape.commas += 1;
            return true;
        }
        false
    }

    pub(super) fn item_body(&mut self, item: Item, shape: &mut Shape) {
        if item == Item::Name {
            if self.at(TokenKind::Ident) || self.at(TokenKind::RawIdent) {
                self.leaf(NodeKind::Name);
            } else {
                let msg = self.found(Msg::new().text("expected a name, found "));
                self.report_missing(errors::SYNTAX_EXPECTED_NAME, msg);
                self.missing(NodeKind::Error);
            }
            return;
        }
        let mark = self.out.mark();
        self.expr(Brace::Continues);
        match item {
            Item::Name => unreachable!("read above"),
            Item::Value | Item::Element => {
                if self.at(TokenKind::Eq) {
                    let op = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::FieldInit, Anchor::Token(op), mark);
                }
            }
            Item::Param => {
                if self.at(TokenKind::Colon) {
                    let op = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::Param, Anchor::Token(op), mark);
                    shape.typed += 1;
                }
            }
            Item::Arm => {
                self.whole_binding(mark);
                if self.at(TokenKind::If) {
                    let guard = self.out.mark();
                    let token = self.bump();
                    self.expr(Brace::Continues);
                    self.out.node(NodeKind::Guard, Anchor::Token(token), guard);
                }
                let anchor = if self.at(TokenKind::FatArrow) {
                    Anchor::Token(self.bump())
                } else {
                    self.report_expected(&[TokenKind::FatArrow]);
                    Anchor::Missing(self.current())
                };
                if matches!(anchor, Anchor::Token(_)) || self.starts_expr() {
                    self.expr(Brace::Continues);
                }
                self.out.node(NodeKind::Arm, anchor, mark);
            }
        }
    }

    /// Doc comments and attributes, each wrapping what `body` parses after them.
    pub(super) fn annotated<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        let mut open = Vec::new();
        loop {
            let mark = self.out.mark();
            match self.peek() {
                TokenKind::DocComment => open.push((NodeKind::Doc, self.bump(), mark)),
                TokenKind::HashLBracket => {
                    let token = self.bump();
                    self.list(List::Items(Item::Value), TokenKind::RBracket);
                    self.close(TokenKind::RBracket, token);
                    open.push((NodeKind::Attr, token, mark));
                }
                _ => break,
            }
        }
        if let Some(&(kind, token, _)) = open.last()
            && (self.peek().closes() || self.at(TokenKind::Eof))
        {
            let what = if kind == NodeKind::Doc { "doc comment" } else { "attribute" };
            let msg =
                self.found(Msg::new().text("expected a declaration after this ").text(what).text(", found "));
            let span = self.tokens.span(token);
            self.report(span, Diagnostic::error(errors::SYNTAX_EXPECTED_DECLARATION, msg));
        }
        let before = self.out.mark();
        let result = body(self);
        if !open.is_empty() && self.out.mark() == before {
            self.missing(NodeKind::Error);
        }
        for (kind, token, mark) in open.into_iter().rev() {
            self.out.node(kind, Anchor::Token(token), mark);
        }
        result
    }
}
