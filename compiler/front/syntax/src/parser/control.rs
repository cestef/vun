//! Functions, brackets, block-like expressions and jumps: the forms chosen after their children.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{ASSIGN, Context, ELEMENT_TYPES, NodeKind, TokenKind};

use super::{Brace, Form, Item, List, MAX_NESTING, Operand, Parser};
use crate::cst::{Anchor, Mark};

impl Parser<'_, '_, '_> {
    /// `(...)`: a tuple or a parenthesized expression; a function type when a parameter is typed or
    /// `->` follows; a function literal when a body follows either.
    pub(super) fn function_or_tuple(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let (open, shape) = self.group_of(List::Items(Item::Param));
        let arrow = self.at(TokenKind::Arrow);
        if arrow {
            let returns = self.out.mark();
            let token = self.bump();
            self.expr(Brace::Ends);
            self.out.node(NodeKind::Returns, Anchor::Token(token), returns);
        }
        let body = brace == Brace::Continues
            && self.at(TokenKind::LBrace)
            && (arrow || shape.items == 0 || shape.typed > 0);
        if body {
            self.block(List::Statements);
        }
        let kind = if body {
            NodeKind::Fn
        } else if arrow || shape.typed > 0 {
            NodeKind::FnType
        } else if shape.single() {
            NodeKind::Paren
        } else {
            NodeKind::Tuple
        };
        self.out.node(kind, Anchor::Token(open), mark);
    }

    /// `[...]`: an array, a repeat, an array type `[N]T`, or the captures of a closure.
    pub(super) fn bracket(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let (open, shape) = self.group_of(List::Items(Item::Element));
        let kind = if shape.form == Form::Repeated {
            NodeKind::ArrayRepeat
        } else if self.at(TokenKind::LParen) {
            let callee = self.out.mark();
            self.function_or_tuple(brace);
            let closure = self.out.single_since(callee) && self.out.last() == Some(NodeKind::Fn);
            if closure {
                NodeKind::Closure
            } else if shape.single() {
                NodeKind::ArrayType
            } else {
                // Any other number of items before a group only makes sense as closure captures.
                self.report_expected(&[TokenKind::LBrace]);
                NodeKind::Error
            }
        } else if shape.single() && ELEMENT_TYPES.contains(&self.peek()) {
            self.prefix(brace);
            NodeKind::ArrayType
        } else {
            NodeKind::Array
        };
        self.out.node(kind, Anchor::Token(open), mark);
    }

    /// Block-like expressions: blocks, `if`, `for`, `match` and labeled loops, with bodies of `list`.
    pub(super) fn control(&mut self, list: List) {
        let mark = self.out.mark();
        let (kind, token) = match self.peek() {
            TokenKind::LBrace => return self.block(list),
            TokenKind::If => {
                let token = self.bump();
                self.head(TokenKind::Eq);
                self.block(list);
                if self.eat(TokenKind::Else) {
                    if self.at(TokenKind::If) { self.control(list) } else { self.block(list) }
                }
                (NodeKind::If, token)
            }
            TokenKind::For => {
                let token = self.bump();
                if !self.at(TokenKind::LBrace) {
                    self.head(TokenKind::For);
                }
                self.block(list);
                (NodeKind::For, token)
            }
            TokenKind::Match => {
                let token = self.bump();
                self.expr(Brace::Ends);
                self.braced(List::Items(Item::Arm));
                (NodeKind::Match, token)
            }
            TokenKind::Label => {
                let token = self.bump();
                self.expect(TokenKind::Colon);
                if matches!(self.peek(), TokenKind::For | TokenKind::LBrace) {
                    self.control(list);
                } else {
                    self.report_expected(&[TokenKind::For, TokenKind::LBrace]);
                    self.missing(NodeKind::Error);
                }
                (NodeKind::Labeled, token)
            }
            _ => unreachable!("control called at a block-like token"),
        };
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// The head of `if` or `for`: a condition, `pattern = value`, or for loops `pattern in value`.
    pub(super) fn head(&mut self, keyword: TokenKind) {
        let mark = self.out.mark();
        self.expr(Brace::Ends);
        self.whole_binding(mark);
        let kind = if self.at(TokenKind::Eq) {
            NodeKind::Test
        } else if keyword == TokenKind::For && self.at_word(Context::ForPattern) {
            NodeKind::In
        } else {
            return;
        };
        let token = self.bump();
        self.expr(Brace::Ends);
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// `n := p` in a pattern position: the pattern since `mark` becomes the name of a binding
    /// whose value is the pattern after `:=`.
    pub(super) fn whole_binding(&mut self, mark: Mark) {
        if self.at(TokenKind::ColonEq) {
            let op = self.bump();
            self.expr(Brace::Ends);
            self.out.node(NodeKind::Binding, Anchor::Token(op), mark);
        }
    }

    /// A block-like expression, possibly under `$` or `unsafe`.
    pub(super) fn block_like(&mut self, list: List) {
        if matches!(self.peek(), TokenKind::Dollar | TokenKind::Unsafe) {
            let mark = self.out.mark();
            let token = self.bump();
            self.nested(|p| p.block_like(list));
            self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
        } else {
            self.control(list);
        }
    }

    pub(super) fn block_like_at(&self) -> bool {
        let mut ahead = 0;
        while matches!(self.nth(ahead), TokenKind::Dollar | TokenKind::Unsafe) {
            ahead += 1;
        }
        matches!(
            self.nth(ahead),
            TokenKind::LBrace | TokenKind::If | TokenKind::For | TokenKind::Match | TokenKind::Label
        )
    }

    pub(super) fn jump(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let token = self.bump();
        let (kind, operand) = match self.kind(token) {
            TokenKind::Return => (NodeKind::Return, Operand::Optional),
            TokenKind::Break => (NodeKind::Break, Operand::Optional),
            TokenKind::Continue => (NodeKind::Continue, Operand::Never),
            TokenKind::Yield => (NodeKind::Yield, Operand::Required),
            TokenKind::Defer => (NodeKind::Defer, Operand::Required),
            _ => unreachable!("jump called at a jump keyword"),
        };
        if matches!(kind, NodeKind::Break | NodeKind::Continue) && self.at(TokenKind::Label) {
            self.leaf(NodeKind::Label);
        }
        match operand {
            Operand::Required if kind == NodeKind::Defer => self.deferred(brace),
            Operand::Required => self.expr(brace),
            Operand::Optional if self.starts_expr() => self.expr(brace),
            Operand::Optional | Operand::Never => {}
        }
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// The statement `defer` runs: an expression, or an assignment, which binds looser than any
    /// operator, so `defer x.* = 0` defers the whole assignment (spec 05, section 3).
    fn deferred(&mut self, brace: Brace) {
        let mark = self.out.mark();
        self.expr(brace);
        if ASSIGN.contains(&self.peek()) {
            let op = self.bump();
            self.expr(brace);
            self.out.node(NodeKind::Assign, Anchor::Token(op), mark);
        }
    }

    /// Runs `body` one level deeper, or skips the operand at the current token when nesting is
    /// too deep.
    pub(super) fn nested(&mut self, body: impl FnOnce(&mut Self)) {
        if self.depth >= MAX_NESTING {
            let msg = Msg::new().text("nesting is deeper than ").num(MAX_NESTING).text(" levels");
            let note = Msg::new().text("each operand and each block is one level");
            let span = self.tokens.span(self.current());
            let d = Diagnostic::error(errors::SYNTAX_NESTING_TOO_DEEP, msg).child(ChildKind::Note, note);
            self.report(span, d);
            self.skip_group();
            return;
        }
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }
}
