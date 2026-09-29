//! Functions, brackets, block-like expressions and jumps: the forms chosen after their children.

use vun_grammar::{
    ASSIGN, Context, ELEMENT_TYPES, LABELED_BODIES, NodeKind, PARTS, TRANSPARENT, TokenKind, UNTERMINATED,
};

use vun_trace::tracing;

use super::{Brace, Form, Item, List, Operand, Parser};
use crate::cst::{Anchor, Mark};
use crate::tokens::TokenId;

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
            self.operand(NodeKind::Returns.operand_level(), Brace::Ends);
            self.out.node(NodeKind::Returns, Anchor::Token(token), returns);
        }
        if brace == Brace::Continues
            && self.at(TokenKind::LBrace)
            && (arrow || shape.items == 0 || shape.typed > 0)
        {
            self.block(List::Statements);
        }
        self.group_node(open, mark, brace);
    }

    /// `[...]`: an array, a repeat, an array type `[N]T`, or the captures of a closure.
    pub(super) fn bracket(&mut self, brace: Brace) {
        let mark = self.out.mark();
        let (open, shape) = self.group_of(List::Items(Item::Element));
        let group = shape.form == Form::Listed && self.at(TokenKind::LParen);
        if group {
            self.function_or_tuple(brace);
        } else if shape.single() && ELEMENT_TYPES.contains(&self.peek()) {
            self.operand(NodeKind::ArrayType.operand_level(), brace);
        }
        // Any other number of items before a group only makes sense as closure captures.
        if self.group_node(open, mark, brace).is_none() && group {
            self.report_expected(&[TokenKind::LBrace]);
        }
    }

    /// Pushes the node of the group `open` opened, its children since `mark`: of the kinds
    /// starting with its token that stand as operands here, the first they spell (`preferred`).
    fn group_node(&mut self, open: TokenId, mark: Mark, brace: Brace) -> Option<NodeKind> {
        let context = brace.context();
        let stands = |k: &NodeKind| !PARTS.contains(k) || context.contains(k);
        let kinds: Vec<NodeKind> =
            NodeKind::starting_with(self.kind(open)).iter().copied().filter(stands).collect();
        let chosen = self.out.node_of(&kinds, Anchor::Token(open), mark, &context, self.tokens, self.source);
        tracing::trace!(?open, token = ?self.kind(open), ?kinds, ?chosen, "group");
        chosen
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
                // `else if` is one level deeper, as a link of the chain of branches.
                if self.eat(TokenKind::Else) {
                    if self.at(TokenKind::If) { self.nested(|p| p.control(list)) } else { self.block(list) }
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
                let bodies = || LABELED_BODIES.iter().flat_map(|k| k.starts());
                if bodies().any(|&k| k == self.peek()) {
                    self.control(list);
                } else {
                    self.report_expected(&bodies().copied().collect::<Vec<_>>());
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

    /// A block-like expression, possibly under a token making a prefix transparent: `$`, `unsafe`.
    pub(super) fn block_like(&mut self, list: List) {
        if Self::makes_transparent(self.peek()) {
            let mark = self.out.mark();
            let token = self.bump();
            self.nested(|p| p.block_like(list));
            self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
        } else {
            self.control(list);
        }
    }

    /// Whether an entry needing no `;` starts here: one of the kinds of `unterminated` holding
    /// children, under tokens making a prefix transparent.
    pub(super) fn block_like_at(&self) -> bool {
        let mut ahead = 0;
        while Self::makes_transparent(self.nth(ahead)) {
            ahead += 1;
        }
        let at = self.nth(ahead);
        UNTERMINATED.iter().any(|k| !k.slots().is_empty() && k.starts().contains(&at))
    }

    /// Whether a prefix written with `token` ends with the kind of its operand.
    fn makes_transparent(token: TokenKind) -> bool {
        TRANSPARENT.iter().any(|row| row.kind == NodeKind::Prefix && row.tokens.contains(&token))
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
            Operand::Required => self.operand(kind.operand_level(), brace),
            Operand::Optional if self.starts_expr() => self.operand(kind.operand_level(), brace),
            Operand::Optional | Operand::Never => {}
        }
        self.out.node(kind, Anchor::Token(token), mark);
    }

    /// The statement `defer` runs: an expression, or an assignment, which binds looser than any
    /// operator, so `defer x.* = 0` defers the whole assignment (spec 05, section 3).
    fn deferred(&mut self, brace: Brace) {
        let mark = self.out.mark();
        self.operand(NodeKind::Defer.operand_level(), brace);
        if ASSIGN.contains(&self.peek()) {
            let op = self.bump();
            self.expr(brace);
            self.out.node(NodeKind::Assign, Anchor::Token(op), mark);
        }
    }
}
