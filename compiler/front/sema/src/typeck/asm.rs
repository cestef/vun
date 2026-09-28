//! A block of instructions (spec 07, section 6): unchecked, so written in `unsafe`; its
//! architecture a namespace of `lang.arch` known at compile time; each value bound in braces
//! typed, an output a place; the block checked against the architecture's table. It is `()`, or
//! `never` when its end is not reached.

use std::collections::BTreeMap;

use vun_diag::{Diagnostic, Msg, errors};
use vun_grammar::Flow;
use vun_hir::tree::{Asm, AsmEntry, ExprId, Operand, OperandKind};

use super::{Known, Stage, Typer};
use crate::asm::{Arch, ArchTable, Checker};
use crate::float::Format;
use crate::layout::Shapes;
use crate::target::TargetOf;
use crate::ty::{TyKind, Value};

impl Typer<'_, '_, '_> {
    pub(super) fn asm(&mut self, expr: ExprId, asm: &Asm) -> Option<Known> {
        self.unchecked(expr, Msg::new().text("a block of instructions"));
        let mut bindings = Vec::new();
        for entry in &asm.entries {
            if let AsmEntry::Inst(inst) = entry {
                inst.operands.iter().for_each(|o| Self::bindings(o, &mut bindings));
            }
        }
        let mut placed = true;
        for &(flow, value) in &bindings {
            self.typed(value);
            if flow.is_some_and(|f| f != Flow::In) && !self.hir().exprs[value].kind.is_place() {
                placed = false;
                let msg = Msg::new()
                    .text("an output is written into a place: a local, a field, an item or ")
                    .code("p.*");
                self.diag.error(
                    Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg)
                        .at(self.hir().exprs[value].origin, None),
                );
            }
        }
        asm.each_expr(&mut |e| {
            if !bindings.iter().any(|&(_, v)| v == e) {
                self.typed(e);
            }
        });
        let unit = Known { ty: TyKind::unit(), stage: Stage::Run };
        let Some((arch, table)) = self.arch(asm.arch).filter(|_| placed) else { return Some(unit) };
        // A value's width is known only where the target's code is the block's: elsewhere the
        // block is checked for its class of register alone.
        let targeted = self.env().cx.get::<TargetOf>(&()).is_some_and(|t| t.isa == arch.namespace().symbol());
        let mut bound = BTreeMap::new();
        for &(flow, value) in &bindings {
            let Some(known) = flow.and(self.found.types.get(&value).copied()) else { continue };
            let bytes = self.env().cx.get::<Shapes>(&known.ty).map(|s| s.layout.size).filter(|_| targeted);
            let float = Format::of(&known.ty).is_some();
            if let Some(register) = arch.register(&table, bytes, float) {
                bound.insert(value, register);
            }
        }
        let block = Checker { arch, table: &table, bound: &bound, diag: &mut self.diag }.block(asm)?;
        let known = if block.returns { unit } else { self.never()? };
        self.found.asm.insert(expr, block);
        Some(known)
    }

    /// The architecture a block names at `at`, and its table; `None` when it is none, which is
    /// reported.
    fn arch(&mut self, at: ExprId) -> Option<(Arch, std::sync::Arc<crate::asm::Table>)> {
        self.typed(at);
        let env = self.env();
        let value = env.eval(at).ok();
        let found = Arch::ALL.iter().copied().find(|arch| match (&value, env.item(arch.namespace())) {
            (Some(Value::Namespace(ns)), Ok(Value::Namespace(declared))) => *ns == declared,
            _ => false,
        });
        let Some(arch) = found else {
            let msg = Msg::new().text("this is not an architecture of ").code("lang.arch");
            self.diag.error(
                Diagnostic::error(errors::ASM_UNKNOWN_ARCH, msg).at(self.hir().exprs[at].origin, None),
            );
            return None;
        };
        let table = self.env().cx.get::<ArchTable>(&(self.env().file.package, arch)).ok()?;
        Some((arch, table))
    }

    /// The expressions `operand` binds in braces, with how: `None` for the address `{name}`.
    fn bindings(operand: &Operand, out: &mut Vec<(Option<Flow>, ExprId)>) {
        match &operand.kind {
            OperandKind::Bound { flow, value, .. } => out.push((*flow, *value)),
            OperandKind::Prefix { operand, .. } => Self::bindings(operand, out),
            OperandKind::Infix { left, right, .. } => {
                Self::bindings(left, out);
                Self::bindings(right, out);
            }
            OperandKind::Memory { parts, .. } => parts.iter().for_each(|p| Self::bindings(p, out)),
            _ => {}
        }
    }
}
