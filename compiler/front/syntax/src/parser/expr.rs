//! Operator expressions and operands.

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::{
    Assoc, BorrowWord, Continuation, Level, MEMBER_NAMES, NodeKind, NodeTerm, OPEN_ENDED, OPERAND_CONTEXTS,
    PREFIX, TokenKind,
};
use vun_trace::tracing;

use super::nesting::Chain;
use super::{Brace, Item, List, Parser};
use crate::cst::Anchor;

impl Parser<'_, '_, '_> {
    pub(super) fn expr(&mut self, brace: Brace) {
        self.operand(Level::LOWEST, brace);
    }

    /// An operand held at `min`: a form starting here, then each form at `min` or tighter
    /// continuing what was read (`precedence`).
    pub(super) fn operand(&mut self, min: Level, brace: Brace) {
        let mark = self.out.mark();
        self.prefix(brace);
        let mut chained = None;
        let mut chain = Chain::new(mark);
        while let Some(kind) = self.continuing(min, brace) {
            tracing::trace!(?kind, ?min, token = ?self.peek(), at = ?self.current(), "continues the operand");
            if !self.link(&mut chain) {
                return;
            }
            let (kind, token) = match kind {
                NodeKind::Asm => {
                    self.asm(mark);
                    continue;
                }
                NodeKind::Member => {
                    let token = self.bump();
                    self.member_name();
                    (kind, token)
                }
                NodeKind::Postfix => (kind, self.bump()),
                NodeKind::Call | NodeKind::TypedLit => (kind, self.group_of(List::Items(Item::Value)).0),
                NodeKind::Index => {
                    let token = self.bump();
                    self.expr(Brace::Continues);
                    self.close(TokenKind::RBracket, token);
                    (kind, token)
                }
                _ if kind == NodeKind::INFIX_FORM => {
                    let Some(op) = self.peek().infix() else { break };
                    let wrong = op.assoc == Assoc::None && chained == Some(op.level);
                    if wrong {
                        self.report_chained();
                    }
                    let token = self.bump();
                    // `a..` has no upper bound.
                    if !OPEN_ENDED.contains(&self.kind(token)) || self.starts_expr() {
                        self.operand(op.level.tighter(), brace);
                    }
                    chained = (op.assoc == Assoc::None).then_some(op.level);
                    // A chain reported is an error node, whose meaning no later stage guesses.
                    (if wrong { NodeKind::Error } else { kind }, token)
                }
                _ => unreachable!("{kind:?} continues an operand, and the parser reads no such form"),
            };
            self.out.node(kind, Anchor::Token(token), mark);
        }
    }

    /// The form continuing the operand just read at the current token: among the forms
    /// continuing an operand (`Continuation`) at `min` or tighter that may hold it, the one whose
    /// leading tokens are all here, the longest such. An instruction's operand ends with its line,
    /// and a `{` continues it only where one continues an expression.
    fn continuing(&self, min: Level, brace: Brace) -> Option<NodeKind> {
        let last = self.out.last()?;
        if !self.line_goes_on(brace) {
            return None;
        }
        let level = |c: &Continuation| {
            if c.kind == NodeKind::INFIX_FORM { self.peek().infix().map(|op| op.level) } else { c.level }
        };
        let here = Continuation::at(self.peek())
            .iter()
            .filter(|c| c.holds(last) && level(c).is_none_or(|l| l >= min));
        let leads = here.filter_map(|c| {
            Some((c.leads.iter().filter_map(|run| self.leads_here(run, brace)).max()?, c.kind))
        });
        leads.max_by_key(|&(length, _)| length).map(|(_, kind)| kind)
    }

    /// The length of `run`, tokens and words, where it is spelled from the current token.
    fn leads_here(&self, run: &[NodeTerm], brace: Brace) -> Option<usize> {
        let spelled = |(ahead, term): (usize, &NodeTerm)| match *term {
            NodeTerm::Token(kinds) => {
                let at = self.nth(ahead);
                kinds.contains(&at) && (at != TokenKind::LBrace || brace == Brace::Continues)
            }
            NodeTerm::Word(words) => self.spelled_at(ahead, words.iter().copied()),
            _ => false,
        };
        (!run.is_empty() && run.iter().enumerate().all(spelled)).then_some(run.len())
    }

    pub(super) fn prefix(&mut self, brace: Brace) {
        self.nested(|p| {
            let k = p.peek();
            if k == TokenKind::HashLBracket {
                p.annotated(|p| p.operand(NodeKind::Attr.operand_level(), brace));
            } else if k == TokenKind::DotDot && !p.starts_expr_at(1) {
                // `..` alone: the rest of a tuple, array or struct in a pattern.
                p.leaf(NodeKind::Rest);
            } else if PREFIX.contains(&k) {
                let mark = p.out.mark();
                let token = p.bump();
                // The words of a borrow mode, a qualifier first: `&temp mut`.
                if k == TokenKind::Amp {
                    for (words, kind) in
                        [(BorrowWord::QUALIFIERS, NodeKind::Qualifier), (BorrowWord::MODES, NodeKind::Mode)]
                    {
                        if p.at_spelled(words.iter().filter_map(|w| w.text())) {
                            p.leaf(kind);
                        }
                    }
                }
                p.operand(NodeKind::Prefix.operand_level(), brace);
                p.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            } else {
                p.primary(brace);
            }
        });
    }

    /// The kind of `operand_contexts` an operand starting with `token` is, among an instruction's
    /// operands: `{` a bound, `[` a memory operand, a label a label.
    fn context_kind(token: TokenKind) -> Option<NodeKind> {
        OPERAND_CONTEXTS.iter().flat_map(|row| row.kinds).copied().find(|k| k.starts().contains(&token))
    }

    pub(super) fn member_name(&mut self) {
        match self.peek() {
            TokenKind::Ident | TokenKind::RawIdent => self.leaf(NodeKind::Name),
            TokenKind::Int => self.leaf(NodeKind::Int),
            TokenKind::Dollar => {
                let mark = self.out.mark();
                let token = self.bump();
                self.nested(Self::computed_name);
                self.out.node(NodeKind::Prefix, Anchor::Token(token), mark);
            }
            _ => {
                let msg = self.found(Msg::new().text("expected a member name, found "));
                self.report_missing(errors::SYNTAX_EXPECTED_NAME, msg);
                self.missing(NodeKind::Error);
            }
        }
    }

    /// The value naming a member after `.$`, of a kind `member_names` lists; another is reported
    /// and held by an `Error`.
    fn computed_name(&mut self) {
        let (mark, at) = (self.out.mark(), self.current());
        self.primary(Brace::Continues);
        if self.out.last().is_some_and(|k| k != NodeKind::Error && !MEMBER_NAMES.contains(&k)) {
            let msg = Msg::new().text("expected a name, an index or a parenthesized value naming a member");
            self.report(self.tokens.span(at), Diagnostic::error(errors::SYNTAX_EXPECTED_NAME, msg));
            // Its tokens anchor the nodes it holds; the error stands where the name was expected.
            self.out.node(NodeKind::Error, Anchor::Missing(at), mark);
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
        if brace == Brace::Hole
            && let Some(kind) = Self::context_kind(self.peek())
        {
            match kind {
                NodeKind::Bound => self.bound(),
                NodeKind::Memory => self.memory(),
                leaf => self.leaf(leaf),
            }
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
