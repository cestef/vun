//! A block of instructions (spec 07, section 6): `lang.arch.asm({ ... })`, a call of `lang.arch.asm` whose
//! code, a block, calls the instructions of one architecture's table; each value bound (`in(e)`)
//! typed, an output a place; the block checked against the table. The call is `()`, or `never`
//! when the end of the block is not reached.

use std::collections::BTreeMap;

use vun_diag::{DiagCtxt, Diagnostic, Msg, errors};
use vun_hir::tree::{Arg, BlockId, Entry, ExprId, ExprKind, Hir};
use vun_hir::visit::{self, Visitor};
use vun_primitives::Primitive;
use vun_primitives::arch::Arch as Form;
use vun_sema_ty::TyKind;
use vun_sema_ty::item::Site;

use super::{Known, Stage, Typer};
use crate::asm::{Arch, ArchTable, Checker, Held, Table};
use crate::callable::Callable;
use crate::layout::Shapes;
use crate::target::TargetOf;

impl Typer<'_, '_, '_> {
    /// The call `call` of `lang.arch.asm` with the code `code`, typed as it was.
    pub(super) fn asm(&mut self, call: ExprId, code: ExprId) -> Option<Known> {
        let unit = Known { ty: TyKind::unit(), stage: Stage::Run };
        let hir = self.hir();
        let ExprKind::Block(block) = hir.exprs[code].kind else {
            let msg = Msg::new()
                .text("a block of instructions is a block of their calls: ")
                .code("lang.arch.asm({ ... })");
            self.diag
                .error(Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg).at(hir.exprs[code].origin, None));
            return Some(unit);
        };
        let calls = Calls::of(hir, block);
        // A block whose calls depend on a `$` parameter is read where it is instantiated, as the
        // calls are typed there.
        if calls.iter().any(|c| !self.found.types.contains_key(&c.expr)) {
            return Some(unit);
        }
        let forms: BTreeMap<ExprId, Form> = calls
            .iter()
            .filter_map(|c| match self.called(c.callee)? {
                Callable::Primitive(Primitive::Arch(form)) => Some((c.callee, form)),
                Callable::Primitive(_) | Callable::Linked(_) | Callable::Fn(_) => None,
            })
            .collect();
        // The values bound to registers, each with the form binding it.
        let bindings: Vec<(Form, ExprId)> = calls
            .iter()
            .filter_map(|c| match forms.get(&c.callee)? {
                form @ (Form::r#in | Form::out | Form::inout) => Some((*form, c.first?)),
                _ => None,
            })
            .collect();
        let mut placed = true;
        for &(form, value) in &bindings {
            if form != Form::r#in && !hir.exprs[value].kind.is_place() {
                placed = false;
                let msg = Msg::new()
                    .text("an output is written into a place: a local, a field, an item or ")
                    .code("p.*");
                self.diag.error(
                    Diagnostic::error(errors::ASM_UNKNOWN_OPERAND, msg).at(hir.exprs[value].origin, None),
                );
            }
        }
        let instructions = calls.iter().map(|c| c.callee).filter(|c| !forms.contains_key(c));
        let Some((arch, table)) = self.arch_of(instructions).filter(|_| placed) else { return Some(unit) };
        // A value's width is known only where the target's code is the block's: elsewhere the
        // block is checked for its class of register alone.
        let targeted = self.env().cx.get::<TargetOf>(&()).is_some_and(|t| t.isa == arch.namespace().symbol());
        let mut bound = BTreeMap::new();
        for &(_, value) in &bindings {
            let Some(known) = self.found.types.get(&value).copied() else { continue };
            let bytes = self.env().cx.get::<Shapes>(&known.ty).map(|s| s.layout.size).filter(|_| targeted);
            bound.insert(value, arch.register(&table, bytes, Held::of(known.ty)));
        }
        let mut diag = DiagCtxt::new();
        let checked = Checker {
            env: self.env(),
            arch,
            table: &table,
            forms: &forms,
            bound: &bound,
            targeted,
            diag: &mut diag,
        }
        .block(block);
        self.diag.absorb(&diag);
        let block = checked?;
        let known = if block.returns { unit } else { self.never()? };
        self.finding().asm.insert(call, block);
        Some(known)
    }

    /// The architecture whose table holds the instructions `callees` name, and the table; `None`
    /// when they are of none, or of several, which is reported.
    fn arch_of(&mut self, callees: impl Iterator<Item = ExprId>) -> Option<(Arch, std::sync::Arc<Table>)> {
        let env = self.env();
        let package = env.file.package;
        let tables: Vec<_> = Arch::ALL
            .iter()
            .filter_map(|&arch| Some((arch, env.cx.get::<ArchTable>(&(package, arch)).ok()?)))
            .collect();
        let mut found: Option<usize> = None;
        for callee in callees {
            let Ok(Site::Decl(decl)) = env.resolve(callee) else { continue };
            let Some(at) = tables.iter().position(|(_, t)| t.callables.contains_key(&decl)) else { continue };
            if found.is_some_and(|f| f != at) {
                let msg =
                    Msg::new().text("this instruction is of another architecture than the block's first");
                let origin = self.hir().exprs[callee].origin;
                self.diag.error(Diagnostic::error(errors::ASM_UNKNOWN_ARCH, msg).at(origin, None));
                return None;
            }
            found = Some(at);
        }
        found.map(|at| tables[at].clone())
    }
}

/// A call a block makes: its expression, its callee, and its first argument written without a
/// name.
struct Call {
    expr: ExprId,
    callee: ExprId,
    first: Option<ExprId>,
}

/// The calls a block makes.
struct Calls(Vec<Call>);

impl Calls {
    fn of(hir: &Hir, block: BlockId) -> Vec<Call> {
        let mut calls = Self(Vec::new());
        calls.block(hir, block);
        calls.0
    }
}

impl Visitor for Calls {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let ExprKind::Call { callee, args } = &hir.exprs[expr].kind {
            let first = match args.first() {
                Some(Entry::One(Arg { name: None, value })) => Some(*value),
                _ => None,
            };
            self.0.push(Call { expr, callee: *callee, first });
        }
        visit::walk_expr(self, hir, expr);
    }
}
