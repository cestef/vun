//! Blocks of an architecture's instructions: each line's words, numbers and bindings, which sema
//! reads against the architecture's table.

use vun_grammar::{Flow, NodeField, NodeKind};
use vun_syntax::cst::{Fields, NodeId};

use super::Lower;
use crate::tree::{Asm, AsmEntry, Inst, Operand, OperandKind, Writeback};

impl Lower<'_, '_> {
    pub(super) fn asm(&mut self, f: &Fields, n: NodeId) -> Asm {
        let arch = self.operand(f, n, NodeField::Target);
        let entries = f.get(NodeField::Lines).iter().map(|&line| self.asm_entry(line)).collect();
        Asm { arch, entries }
    }

    fn asm_entry(&mut self, n: NodeId) -> AsmEntry {
        if self.kind(n) == NodeKind::Label {
            return AsmEntry::Label { name: self.label(n), origin: self.origin(n) };
        }
        let f = self.fields(n);
        let suffix = f.one(NodeField::Suffix).and_then(|s| self.fields(s).one(NodeField::Name));
        AsmEntry::Inst(Inst {
            mnemonic: self.symbol(n),
            suffix: suffix.map(|s| self.symbol(s)),
            operands: f.get(NodeField::Operands).iter().map(|&o| self.asm_operand(o)).collect(),
            origin: self.origin(n),
        })
    }

    fn asm_operand(&mut self, n: NodeId) -> Operand {
        let kind = self.asm_operand_kind(n);
        Operand { kind, origin: self.origin(n) }
    }

    fn asm_operand_kind(&mut self, n: NodeId) -> OperandKind {
        let f = self.fields(n);
        let boxed = |this: &mut Self, field| f.one(field).map(|o| Box::new(this.asm_operand(o)));
        match self.kind(n) {
            NodeKind::Name => OperandKind::Word(self.symbol(n)),
            NodeKind::Int => {
                vun_syntax::literal::integer(self.text(n)).map_or(OperandKind::Error, OperandKind::Int)
            }
            NodeKind::Label => OperandKind::Label(self.label(n)),
            NodeKind::Paren => {
                f.one(NodeField::Inner).map_or(OperandKind::Error, |i| self.asm_operand_kind(i))
            }
            NodeKind::Prefix => match (self.token(n), boxed(self, NodeField::Operand)) {
                (Some(op), Some(operand)) => OperandKind::Prefix { op, operand },
                _ => OperandKind::Error,
            },
            NodeKind::Infix => {
                match (self.token(n), boxed(self, NodeField::Left), boxed(self, NodeField::Right)) {
                    (Some(op), Some(left), Some(right)) => OperandKind::Infix { op, left, right },
                    _ => OperandKind::Error,
                }
            }
            NodeKind::Memory => OperandKind::Memory {
                parts: f.get(NodeField::Parts).iter().map(|&p| self.asm_operand(p)).collect(),
                writeback: if f.one(NodeField::Writeback).is_some() { Writeback::Yes } else { Writeback::No },
            },
            NodeKind::Bound => OperandKind::Bound {
                flow: f.one(NodeField::Flow).and_then(|w| Flow::spelled(self.text(w))),
                register: f.one(NodeField::Register).map(|r| self.symbol(r)),
                value: self.operand(&f, n, NodeField::Value),
            },
            NodeKind::Error => OperandKind::Error,
            _ => OperandKind::Unfit(self.expr(n)),
        }
    }
}
