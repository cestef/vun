//! Operator expressions and operands.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::{Assoc, Context, LITERAL_TYPES, NodeKind, POSTFIX, PREFIX, Prec, TokenKind};

use super::nesting::Chain;
use super::{Brace, Item, List, Parser};
use crate::cst::Anchor;

impl Parser<'_, '_, '_> {
    pub(super) fn expr(&mut self, brace: Brace) {
        self.binary(Prec::LOWEST, brace);
    }

    pub(super) fn binary(&mut self, min: Prec, brace: Brace) {
        let mark = self.out.mark();
        self.prefix(brace);
        let mut chained = None;
        let mut chain = Chain::new(mark);
        while let Some(op) = self.peek().infix().filter(|op| op.prec >= min) {
            if !self.link(&mut chain) {
                return;
            }
            let wrong = op.assoc == Assoc::None && chained == Some(op.prec);
            if wrong {
                self.report_chained();
            }
            let token = self.bump();
            // `a..` has no upper bound.
            if self.kind(token) != TokenKind::DotDot || self.starts_expr() {
                self.binary(op.prec.tighter(), brace);
            }
            // A chain reported is an error node, whose meaning no later stage guesses.
            let kind = if wrong { NodeKind::Error } else { NodeKind::Infix };
            self.out.node(kind, Anchor::Token(token), mark);
            chained = (op.assoc == Assoc::None).then_some(op.prec);
        }
    }

    pub(super) fn prefix(&mut self, brace: Brace) {
        self.nested(|p| {
            let k = p.peek();
            if k == TokenKind::HashLBracket {
                p.annotated(|p| p.prefix(brace));
            } else if k == TokenKind::DotDot && !p.starts_expr_at(1) {
                // `..` alone: the rest of a tuple, array or struct in a pattern.
                p.leaf(NodeKind::Rest);
            } else if PREFIX.contains(&k) {
                let mark = p.out.mark();
                let token = p.bump();
                if k == TokenKind::Amp {
                    if p.at_word(Context::BorrowQualifier) {
                        p.leaf(NodeKind::Mode);
                    }
                    if p.at(TokenKind::Mut) || p.at_word(Context::AfterAmp) {
                        p.leaf(NodeKind::Mode);
                    }
                }
                p.prefix(brace);
                p.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            } else {
                p.postfix(brace);
            }
        });
    }

    pub(super) fn postfix(&mut self, brace: Brace) {
        let mark = self.out.mark();
        self.primary(brace);
        let mut chain = Chain::new(mark);
        loop {
            let (kind, token) = match self.peek() {
                TokenKind::Dot => {
                    let token = self.bump();
                    self.member_name();
                    (NodeKind::Member, token)
                }
                k if POSTFIX.contains(&k) => (NodeKind::Postfix, self.bump()),
                TokenKind::LParen => (NodeKind::Call, self.group_of(List::Items(Item::Value)).0),
                TokenKind::LBracket => {
                    let token = self.bump();
                    self.expr(Brace::Continues);
                    self.close(TokenKind::RBracket, token);
                    (NodeKind::Index, token)
                }
                TokenKind::LBrace
                    if brace == Brace::Continues
                        && self.out.last().is_some_and(|k| LITERAL_TYPES.contains(&k)) =>
                {
                    (NodeKind::TypedLit, self.group_of(List::Items(Item::Value)).0)
                }
                _ => break,
            };
            self.out.node(kind, Anchor::Token(token), mark);
            if !self.link(&mut chain) {
                return;
            }
        }
    }

    pub(super) fn member_name(&mut self) {
        match self.peek() {
            TokenKind::Ident | TokenKind::RawIdent => self.leaf(NodeKind::Name),
            TokenKind::Int => self.leaf(NodeKind::Int),
            TokenKind::Dollar => {
                let mark = self.out.mark();
                let token = self.bump();
                self.nested(|p| p.primary(Brace::Continues));
                self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            }
            _ => {
                let msg = self.found(Msg::new().text("expected a member name, found "));
                self.report_missing(errors::SYNTAX_EXPECTED_NAME, msg);
                self.missing(NodeKind::Error);
            }
        }
    }

    pub(super) fn primary(&mut self, brace: Brace) {
        let leaf = match self.peek() {
            TokenKind::Ident | TokenKind::RawIdent => Some(NodeKind::Name),
            TokenKind::Underscore => Some(NodeKind::Hole),
            TokenKind::Int => Some(NodeKind::Int),
            TokenKind::Float => Some(NodeKind::Float),
            TokenKind::Char => Some(NodeKind::Char),
            TokenKind::String | TokenKind::RawString => Some(NodeKind::String),
            TokenKind::Error => Some(NodeKind::Error),
            _ => None,
        };
        if let Some(kind) = leaf {
            self.leaf(kind);
            return;
        }
        let mark = self.out.mark();
        let (kind, token) = match self.peek() {
            TokenKind::RawStringLine => {
                let first = self.bump();
                while self.at(TokenKind::RawStringLine) {
                    self.leaf(NodeKind::RawStringLine);
                }
                (NodeKind::MultilineString, first)
            }
            TokenKind::Dot => {
                let token = self.bump();
                self.member_name();
                (NodeKind::DotName, token)
            }
            TokenKind::DotLBrace => (NodeKind::StructLit, self.group_of(List::Items(Item::Value)).0),
            TokenKind::HashLBrace => (NodeKind::Quote, self.group_of(List::Statements).0),
            TokenKind::LParen => return self.function_or_tuple(brace),
            TokenKind::LBracket => return self.bracket(brace),
            TokenKind::LBrace | TokenKind::If | TokenKind::For | TokenKind::Match | TokenKind::Label => {
                return self.control(List::Statements);
            }
            TokenKind::Struct => {
                let token = self.bump();
                self.braced(List::Members);
                (NodeKind::Struct, token)
            }
            TokenKind::Enum => {
                let token = self.bump();
                self.enum_body();
                (NodeKind::Enum, token)
            }
            TokenKind::Return
            | TokenKind::Break
            | TokenKind::Continue
            | TokenKind::Yield
            | TokenKind::Defer => return self.jump(brace),
            _ => {
                let msg = self.found(Msg::new().text("expected an expression, found "));
                let mut d = Diagnostic::error(errors::SYNTAX_EXPECTED_EXPRESSION, msg);
                if let Some(keyword) = self.nearby_keyword() {
                    d = d.fix(self.escape(keyword));
                }
                self.report(self.gap(), d);
                self.missing(NodeKind::Error);
                return;
            }
        };
        self.out.node(kind, Anchor::Token(token), mark);
    }
}
