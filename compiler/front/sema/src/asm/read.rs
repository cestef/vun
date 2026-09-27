//! Reading a line's operands: its words as the architecture's registers, integers as
//! immediates, labels as the lines they mark, bindings with the register their value needs, and
//! memory from the terms of its parts.

use std::collections::BTreeMap;

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::{Flow as Binding, TokenKind};
use vun_hir::tree::{ExprId, Operand, OperandKind};
use vun_inline::{Bound, Line};
use vun_intern::Symbol;
use vun_origin::OriginId;

use super::check::{Checker, Term};
use super::given::{Given, Word};

impl Checker<'_> {
    /// Reports an operand the architecture does not have.
    fn unknown(&mut self, origin: OriginId, msg: Msg, help: Option<String>) -> Option<Given> {
        let mut d = Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg).at(origin, None);
        if let Some(near) = help {
            d = d.child(ChildKind::Help, Msg::did_you_mean(near));
        }
        self.diag.error(d);
        None
    }

    /// The operand `operand` gives, or `None` when it is none, which is reported.
    pub(super) fn given(&mut self, operand: &Operand, labels: &BTreeMap<Symbol, Line>) -> Option<Given> {
        match &operand.kind {
            OperandKind::Error => None,
            OperandKind::Word(word) => match self.table.words.get(word) {
                Some(&Word::Reg { reg, size }) => Some(Given::Reg { reg, size }),
                None => {
                    let near =
                        vun_suggest::closest(word.as_str(), self.table.words.keys().map(|w| w.as_str()));
                    let msg = Msg::new().code(word.as_str()).text(" is no register of the architecture");
                    self.unknown(operand.origin, msg, near.map(str::to_owned))
                }
            },
            OperandKind::Int(n) => Some(Given::Imm(i64::try_from(n).ok())),
            OperandKind::Prefix { op: TokenKind::Minus, operand: inner } => match &inner.kind {
                OperandKind::Int(n) => Some(Given::Imm(i64::try_from(-n).ok())),
                _ => self.unfit(operand.origin),
            },
            OperandKind::Label(name) => {
                if let Some(&line) = labels.get(name) {
                    return Some(Given::Label(line));
                }
                let msg = Msg::new().text("the block marks no label named ").code(name.as_str());
                self.unknown(operand.origin, msg, None)
            }
            OperandKind::Bound { flow: None, register: None, value } => Some(Given::Symbol(*value)),
            OperandKind::Bound { flow: Some(flow), register, value } => {
                self.bound(operand, *flow, *register, *value)
            }
            OperandKind::Memory { parts, writeback } => {
                let mut terms = Vec::with_capacity(parts.len());
                for part in parts {
                    let mut out = Vec::new();
                    self.terms(part, 1, &mut out, labels)?;
                    terms.push(out);
                }
                if let Some(mem) = self.arch.memory(terms, *writeback) {
                    return Some(Given::Mem(mem));
                }
                let msg = Msg::new().text("the architecture has no memory operand of this shape");
                self.unknown(operand.origin, msg, None)
            }
            OperandKind::Bound { .. }
            | OperandKind::Unfit(_)
            | OperandKind::Prefix { .. }
            | OperandKind::Infix { .. } => self.unfit(operand.origin),
        }
    }

    fn unfit(&mut self, origin: OriginId) -> Option<Given> {
        let msg = Msg::new().text("this is not an operand of an instruction");
        let help = Msg::new().text("a Vun value goes in braces: ").code("{in e}");
        let d =
            Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg).at(origin, None).child(ChildKind::Help, help);
        self.diag.error(d);
        None
    }

    /// `{flow(register) value}`: in the register named, of its class and width, or in one of the
    /// class and width the value's type needs.
    fn bound(
        &mut self,
        operand: &Operand,
        flow: Binding,
        register: Option<Symbol>,
        value: ExprId,
    ) -> Option<Given> {
        let named = match register {
            Some(name) => match self.table.words.get(&name) {
                Some(&Word::Reg { reg, size }) => Some((reg, size)),
                None => {
                    let msg = Msg::new().code(name.as_str()).text(" is no register of the architecture");
                    return self.unknown(operand.origin, msg, None);
                }
            },
            None => None,
        };
        let reg = named.map(|(reg, _)| reg);
        let (class, size) = match named {
            Some((reg, size)) => (reg.class, size.or_else(|| self.bound.get(&value)?.1)),
            None => *self.bound.get(&value)?,
        };
        let bound = match flow {
            Binding::In => Bound::In { value, reg },
            Binding::Out => Bound::Out { place: value, reg },
            Binding::Inout => Bound::InOut { place: value, reg },
        };
        Some(Given::Bound { bound, class, size })
    }

    /// The terms of a sum in an address, each with its sign.
    fn terms(
        &mut self,
        operand: &Operand,
        sign: i64,
        out: &mut Vec<Term>,
        labels: &BTreeMap<Symbol, Line>,
    ) -> Option<()> {
        match &operand.kind {
            OperandKind::Infix { op: op @ (TokenKind::Plus | TokenKind::Minus), left, right } => {
                self.terms(left, sign, out, labels)?;
                let sign = if *op == TokenKind::Minus { -sign } else { sign };
                self.terms(right, sign, out, labels)
            }
            OperandKind::Infix { op: TokenKind::Star, left, right } if sign == 1 => {
                let (reg, scale) = match (&left.kind, &right.kind) {
                    (_, OperandKind::Int(n)) => (left, n),
                    (OperandKind::Int(n), _) => (right, n),
                    _ => return self.unfit(operand.origin).map(|_| ()),
                };
                let reg = self.given(reg, labels)?;
                out.push(Term::Scaled(reg, i64::try_from(scale).ok()?));
                Some(())
            }
            _ => {
                let term = match self.given(operand, labels)? {
                    Given::Imm(Some(n)) => Term::Bytes(n.checked_mul(sign)?),
                    Given::Symbol(s) if sign == 1 => Term::Symbol(s),
                    given @ (Given::Reg { .. } | Given::Bound { .. }) if sign == 1 => Term::Reg(given),
                    _ => return self.unfit(operand.origin).map(|_| ()),
                };
                out.push(term);
                Some(())
            }
        }
    }
}
