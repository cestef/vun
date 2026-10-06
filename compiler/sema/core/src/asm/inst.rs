//! An instruction called in a block: the instructions its callee names, the parts it writes before
//! its operands selecting their forms, and the first form, in order, that takes its operands, as
//! the native assemblers choose; where none does, why the nearest one does not: an immediate too
//! wide, a register of another class, or no form at all.

use vun_diag::{DiagId, Diagnostic, Msg, errors};
use vun_hir::tree::ExprId;
use vun_inline::Inst;
use vun_inline::Operand;
use vun_isa::{Misfit, Scope, Slot, Stack, Why};
use vun_origin::OriginId;
use vun_sema_float::Format;
use vun_sema_ty::Ty;
use vun_sema_ty::Value;
use vun_sema_ty::float::FloatType;
use vun_sema_ty::item::Site;

use super::check::Checker;
use super::parts::Parts;
use super::read::Labels;
use super::table::declared;
use crate::layout::Shapes;

impl Checker<'_, '_, '_> {
    /// The instruction the call `call` is, or `None` when it is none, which is reported.
    pub(super) fn inst(
        &mut self,
        call: ExprId,
        labels: &Labels,
    ) -> Option<(Inst<ExprId, ExprId, ExprId>, Option<Stack>)> {
        let (callee, args) = self.args(call)?;
        let origin = self.env.parsed.hir.exprs[call].origin;
        let ops = match self.env.resolve(callee) {
            Ok(Site::Decl(decl)) => self.table.callables.get(&decl).cloned(),
            _ => None,
        };
        let Some(ops) = ops else {
            let msg = Msg::new().text("this calls no instruction of the architecture");
            self.diag.error(Diagnostic::error(errors::ASM_UNKNOWN_MNEMONIC, msg).at(origin, None));
            return None;
        };
        let (parts, operands) = self.parts(&args);
        let givens: Vec<_> = operands.iter().map(|&o| self.given(o, labels)).collect();
        let givens = givens.into_iter().collect::<Option<Vec<_>>>()?;
        let widths = &self.arch.shape().widths;
        let width_of = |ty| {
            let bytes = self.env.cx.get::<Shapes>(&ty).map(|s| s.layout.size)?;
            self.table.width(bytes)
        };
        let mut nearest: Option<Misfit> = None;
        for &op in &ops {
            for (form, fixed) in self.table.selected(widths, op, &parts, width_of) {
                match self.arch.fit(op, form, fixed, &givens) {
                    Ok(fit) => {
                        let (size, operands) = (fit.size, fit.operands);
                        vun_trace::tracing::trace!(op = op.0, form = form.0, size = size.0, "fitted");
                        let stack = match fit.stack {
                            Some(stack) if stack.scope == Scope::Call => self.called(&operands, stack),
                            stack => stack,
                        };
                        return Some((Inst { op, form, size, operands, origin }, stack));
                    }
                    Err(misfit) => {
                        vun_trace::tracing::trace!(op = op.0, form = form.0, ?fixed, ?misfit, "missed");
                        nearest = Some(nearest.map_or(misfit, |n| n.nearer(misfit)));
                    }
                }
            }
        }
        let name = ops.first().and_then(|op| self.table.ops.get(usize::from(op.0))).map(|op| op.name);
        self.misfit(origin, name.map_or("", |n| n.as_str()), &operands, nearest.unwrap_or(Misfit::Count));
        None
    }

    /// The stack a call of the function `operands` name makes: its parameters popped and its result
    /// pushed, each of the class a value of its type is held in. `None` where the function's type is
    /// not known, which leaves the rest of the block unchecked.
    fn called(&self, operands: &[Operand<ExprId, ExprId, ExprId>], mut stack: Stack) -> Option<Stack> {
        let symbol = operands.iter().find_map(|o| match o {
            Operand::Symbol(symbol) => Some(*symbol),
            _ => None,
        })?;
        let value = self.env.eval(symbol).ok()?;
        let signature = self.env.value_type(&value).ok()?.signature()?;
        // A type that holds no bytes is passed in no local.
        let slot = |ty: Ty| {
            if !self.targeted {
                return Some(Slot::Any);
            }
            let bytes = self.env.cx.get::<Shapes>(&ty).map(|s| s.layout.size)?;
            if bytes == 0 {
                return None;
            }
            let (class, _) = self.arch.register(self.table, Some(bytes), Format::of(&ty).is_some())?;
            Some(Slot::Class(class))
        };
        // What the form pops itself, a table's index, is above the parameters.
        let above = std::mem::take(&mut stack.pops);
        stack.pops = signature.params.iter().filter_map(|&ty| slot(ty)).chain(above).collect();
        stack.pushes = slot(signature.result).into_iter().collect();
        Some(stack)
    }

    /// The parts `args` write before the operands, and the operands: each leading type, and a
    /// width of encoding where the architecture's forms have one.
    fn parts(&self, args: &[ExprId]) -> (Parts, Vec<ExprId>) {
        let encoding = self.arch.shape().widths.encoding.and_then(|(e, _)| declared(self.env, e).ok());
        let mut parts = Parts::default();
        let mut at = 0;
        while let Some(&arg) = args.get(at) {
            match self.env.eval(arg) {
                Ok(Value::Type(ty)) => parts.types.push(ty),
                Ok(Value::Variant { ty, name, .. }) if Some(ty) == encoding && parts.encoding.is_none() => {
                    parts.encoding = Some(name);
                }
                _ => break,
            }
            at += 1;
        }
        (parts, args[at..].to_vec())
    }

    /// Reports why the nearest form of `name`, called at `origin`, does not take `operands`.
    fn misfit(&mut self, origin: OriginId, name: &str, operands: &[ExprId], misfit: Misfit) {
        let at = |at: usize| operands.get(at).map_or(origin, |&o| self.env.parsed.hir.exprs[o].origin);
        let (id, origin, msg): (DiagId, OriginId, Msg) = match misfit {
            Misfit::Operand { at: operand, why: Why::Wide } => {
                let msg = Msg::new().text("the integer does not fit any immediate of ").code(name);
                (errors::ASM_IMMEDIATE_TOO_WIDE, at(operand), msg)
            }
            Misfit::Operand { at: operand, why: Why::Class } => {
                let msg =
                    Msg::new().text("the register is of another class than ").code(name).text(" takes here");
                (errors::ASM_REGISTER_CLASS, at(operand), msg)
            }
            Misfit::Unsized => {
                let msg = Msg::new().text("no operand gives the width of ").code(name);
                (errors::ASM_NO_FORM, origin, msg)
            }
            Misfit::Count | Misfit::Width | Misfit::Operand { .. } => {
                let msg = Msg::new().text("no form of ").code(name).text(" takes these operands");
                (errors::ASM_NO_FORM, origin, msg)
            }
        };
        self.diag.error(Diagnostic::error(id, msg).at(origin, None));
    }
}

/// The nearer of two misses.
trait Nearer {
    fn nearer(self, other: Self) -> Self;
}

impl Nearer for Misfit {
    /// One at an operand, of the form taking the most operands before it, then the nearest
    /// reason; then one whose width is unknown.
    fn nearer(self, other: Self) -> Self {
        match (self, other) {
            (Self::Operand { at: a, why: wa }, Self::Operand { at: b, why: wb }) => {
                if (b, std::cmp::Reverse(wb)) > (a, std::cmp::Reverse(wa)) { other } else { self }
            }
            (Self::Operand { .. } | Self::Unsized, _) => self,
            (_, Self::Operand { .. } | Self::Unsized) => other,
            _ => self,
        }
    }
}
