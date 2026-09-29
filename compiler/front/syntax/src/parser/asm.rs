//! Blocks of an architecture's instructions (spec 07, section 6): `arch.asm { lines }`, an
//! instruction per line, its mnemonic first and its operands after it, separated by `,`. An
//! operand is an expression of the architecture's operands, where `{` opens a Vun value bound to
//! it: `{in e}`, `{out p}`, `{inout p}`, `{in(rdi) e}`, or `{name}`. A label `'name:` marks the
//! instruction after it, which `'name` among operands targets.

use vun_diag::Msg;
use vun_diag::errors;
use vun_grammar::{Context, Flow, NodeKind, Prec, TokenKind};

use super::{Brace, Parser};
use crate::cst::Anchor;
use crate::tokens::TokenId;

impl Parser<'_, '_, '_> {
    /// Whether the tokens from the current one are `.asm {`.
    pub(super) fn asm_at(&self) -> bool {
        self.at(TokenKind::Dot)
            && self.nth(1) == TokenKind::Ident
            && Context::AsmBlock.keyword(self.text(self.current_plus(1)))
            && self.nth(2) == TokenKind::LBrace
    }

    /// `.asm { lines }` after the architecture's path, which `mark` starts: the `.` anchors the
    /// block, as it anchors a member.
    pub(super) fn asm(&mut self, mark: crate::cst::Mark) {
        let dot = self.bump();
        self.leaf(NodeKind::Punct);
        let open = self.current();
        self.leaf(NodeKind::Punct);
        while !self.at(TokenKind::RBrace) && !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Label) {
                self.leaf(NodeKind::Label);
                self.expect(TokenKind::Colon);
            } else {
                self.instruction();
            }
        }
        self.close(TokenKind::RBrace, open);
        self.out.node(NodeKind::Asm, Anchor::Token(dot), mark);
    }

    /// One line: its mnemonic, which anchors it, a suffix after each `.` (`b.eq`, `vcvt.s32.f32`),
    /// and its operands up to the line's end.
    fn instruction(&mut self) {
        let mark = self.out.mark();
        // A mnemonic may be spelled as a keyword: `and`, `or`.
        if !self.at(TokenKind::Ident) && !self.peek().is_keyword() {
            let msg = self.found(Msg::new().text("expected an instruction, found "));
            self.report_here(errors::SYNTAX_EXPECTED_NAME, msg);
            self.leaf(NodeKind::Error);
            return;
        }
        let mnemonic = self.bump();
        while self.at(TokenKind::Dot) && self.nth(1) == TokenKind::Ident {
            let (mark, dot) = (self.out.mark(), self.bump());
            self.leaf(NodeKind::Name);
            self.out.node(NodeKind::Suffix, Anchor::Token(dot), mark);
        }
        let mut first = true;
        while !self.at(TokenKind::RBrace)
            && !self.at(TokenKind::Eof)
            && !self.line_ends_before(self.current())
        {
            if !first {
                self.expect(TokenKind::Comma);
            }
            first = false;
            let before = self.pos;
            self.binary(Prec::LOWEST, Brace::Hole);
            // What starts no operand is skipped, so every line ends.
            if self.pos == before {
                self.leaf(NodeKind::Error);
            }
        }
        self.out.node(NodeKind::Line, Anchor::Token(mnemonic), mark);
    }

    /// `{flow(register) value}`, a Vun value bound to an operand.
    pub(super) fn bound(&mut self) {
        let mark = self.out.mark();
        let open = self.bump();
        if self.at_spelled(Flow::FLOWS.iter().filter_map(|f| f.text())) && self.nth(1) != TokenKind::RBrace {
            self.leaf(NodeKind::Flow);
            if self.at(TokenKind::LParen) {
                let paren = self.current();
                self.leaf(NodeKind::Punct);
                self.member_name();
                self.close(TokenKind::RParen, paren);
            }
        }
        self.expr(Brace::Continues);
        self.close(TokenKind::RBrace, open);
        self.out.node(NodeKind::Bound, Anchor::Token(open), mark);
    }

    /// `[a, b]` among an instruction's operands: the parts of a memory operand, each an operand,
    /// and `!` after it when the access writes the address back to its base.
    pub(super) fn memory(&mut self) {
        let mark = self.out.mark();
        let open = self.bump();
        while !self.at(TokenKind::RBracket) && !self.at(TokenKind::Eof) {
            let before = self.pos;
            self.binary(Prec::LOWEST, Brace::Hole);
            if self.pos == before || !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.close(TokenKind::RBracket, open);
        if self.at(TokenKind::Bang) && !self.line_ends_before(self.current()) {
            self.leaf(NodeKind::Writeback);
        }
        self.out.node(NodeKind::Memory, Anchor::Token(open), mark);
    }

    /// Whether the expression parsed goes on at the current token: always, but in an
    /// instruction's operand, which its line ends.
    pub(super) fn line_goes_on(&self, brace: Brace) -> bool {
        brace != Brace::Hole || !self.line_ends_before(self.current())
    }

    /// Whether a line break stands before the token `t`.
    fn line_ends_before(&self, t: TokenId) -> bool {
        self.tokens
            .leading_trivia(t)
            .any(|l| l.kind == TokenKind::Whitespace && l.range.of(self.source).contains('\n'))
    }
}
