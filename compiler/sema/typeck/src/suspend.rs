//! `yield v` and `yield ..e` (spec 06): a value yielded is checked against what the coroutine
//! function yields; delegating yields what `e`'s frame yields and has what it returns.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_grammar::TokenKind;
use vun_hir::tree::{ExprId, ExprKind};
use vun_sema_ty::{Ty, TyKind, Value};

use super::{Known, Stage, Typer};

impl Typer<'_, '_, '_> {
    /// Reports the coroutine typed when `delegated`, a frame built in place inside its own, is a
    /// frame of the same function: the frame would contain itself (spec 06, section 1).
    fn recursive_frame(&mut self, delegated: Ty) {
        let (Some(literal), TyKind::Frame(frame)) = (self.framing, &*delegated) else { return };
        if frame.literal.expr.id != literal || frame.literal.file != self.env().file {
            return;
        }
        let msg = Msg::new().text("this coroutine's frame would contain itself, through ").code("yield ..");
        let help =
            Msg::new().text("recurse in an ordinary function, or hold each nested frame in an ").code("Own");
        let d = Diagnostic::error(errors::COROUTINE_RECURSIVE_FRAME, msg)
            .at(self.hir().exprs[literal].origin, None)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    /// Reports a function annotated with an interface that never yields, `found` the type of its
    /// body: it has no frame to return, so its body must not finish, as a task that loops
    /// forever (spec 06, section 1).
    pub(super) fn unyielding(&mut self, returns: ExprId, found: Option<Known>) {
        let finishes = found.is_some_and(|f| Some(f.ty) != self.never().map(|n| n.ty));
        if !finishes || !matches!(self.annotated(returns), Ok(Value::Interface(_))) {
            return;
        }
        let msg = Msg::new().text("this function returns an interface but never yields");
        let help = Msg::new().text("yield from the body, or return a type");
        let d = Diagnostic::error(errors::COROUTINE_NOT_SUSPENDING, msg)
            .at(self.hir().exprs[returns].origin, None)
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }

    pub(super) fn yielded(&mut self, value: ExprId) -> Option<Known> {
        let hir = self.hir();
        if let ExprKind::Prefix { op: TokenKind::DotDot, operand, .. } = hir.exprs[value].kind {
            let delegated = self.typed(operand)?;
            self.recursive_frame(delegated.ty);
            let (yields, returns) = self.env().frame_step(delegated.ty)?;
            if let Some(expected) = self.yields
                && !expected.accepts(&yields)
            {
                self.expect(hir.exprs[operand].origin, Known { ty: yields, ..delegated }, expected, None);
            }
            return Some(Known { ty: returns, stage: self.stage });
        }
        let found = self.typed_as(value, self.yields)?;
        if let Some(expected) = self.yields {
            self.expect(hir.exprs[value].origin, found, expected, None);
        }
        Some(Known { ty: TyKind::unit(), stage: Stage::Compile })
    }
}
