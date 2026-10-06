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

use super::{Known, Stage, Typer};
use crate::asm::{Assembled, Assemblers, Request};
use crate::callable::Callable;

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
        let instructions = calls.iter().map(|c| c.callee).filter(|c| !forms.contains_key(c)).collect();
        let bound = bindings
            .iter()
            .filter_map(|&(_, value)| Some((value, self.found.types.get(&value)?.ty)))
            .collect();
        let request = Request { block, instructions, forms, bound, placed };
        let Some(assembler) = self.env().cx.db().resource::<Assemblers>().get().cloned() else {
            return Some(unit);
        };
        let mut diag = DiagCtxt::new();
        let assembled = assembler.assemble(self.env(), &request, &mut diag);
        self.diag.absorb(&diag);
        let block = match assembled {
            Assembled::Skipped => return Some(unit),
            Assembled::Rejected => return None,
            Assembled::Checked(block) => block,
        };
        let known = if block.returns { unit } else { self.never()? };
        self.finding().asm.insert(call, block);
        Some(known)
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
