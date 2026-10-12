//! The bounds a call's `$` parameters are given: `$T: I` is met by a type with an impl of `I`
//! (spec 02, section 3), and `$F: Call((T), R)` by a function or closure of that signature.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_hir::resolve::Res;
use vun_hir::tree::ExprKind;
use vun_sema_ty::call::{Func, Instantiation};
use vun_sema_ty::{Ty, Value};

use super::Argument;
use crate::Typer;
use vun_hir::tree::ExprId;
use vun_origin::OriginTable;
use vun_sema::callable::Written;
use vun_sema::env::Env;

impl Typer<'_, '_, '_> {
    /// Reports each `$` parameter of the call at `call` that the type bound to it does not meet
    /// the interface annotating it for. `slots` are the parameters the written `args` are for.
    pub(super) fn check_bounds(
        &mut self,
        call: ExprId,
        instantiation: &Instantiation,
        slots: &[usize],
        args: &[Argument],
        inferred: &[usize],
    ) {
        let f = &instantiation.f;
        // The parameters the literal writes itself, in its own file: a member leaving some types
        // out has them written by its requirement (`Parameters::written`), in another file.
        let Some(own) = Env::inside(self.env().cx, f, []) else { return };
        let written = Written { env: own, expr: f.literal.expr.id };
        let Some(params) = written.params() else { return };
        let Some(env) = Env::inside(self.env().cx, f, instantiation.args.iter().cloned()) else { return };
        let hir = &env.parsed.hir;
        for (i, (binder, annotation)) in params.iter().enumerate() {
            let (Some(binder), Some(annotation)) = (binder, annotation) else { continue };
            let Some(Some(Value::Type(ty))) = instantiation.args.get(i) else { continue };
            if !hir.binders[*binder].comptime {
                continue;
            }
            let Ok(Value::Interface(interface)) = env.eval(*annotation) else { continue };
            if env.satisfies(*ty, &interface, inferred.iter().all(|&slot| slot == i)) {
                continue;
            }
            // The argument whose parameter is annotated with this one, if one is written.
            let by = params.iter().position(|(_, a)| {
                a.is_some_and(|a| {
                    matches!(hir.exprs[a].kind, ExprKind::Name(_))
                        && env.parsed.resolution.names[a] == Some(Res::Binder(*binder))
                })
            });
            let at = by
                .and_then(|j| slots.iter().position(|&s| s == j))
                .and_then(|k| match args.get(k) {
                    Some(Argument::Expr(e)) => Some(self.hir().exprs[*e].origin),
                    _ => None,
                })
                .unwrap_or(self.hir().exprs[call].origin);
            self.unmet_bound(at, *ty, f, hir.binders[*binder].name.as_str(), &interface);
        }
    }

    /// Reports `ty` as not meeting the bound `interface` of the `$` parameter `name` of `f`.
    fn unmet_bound(&mut self, at: vun_origin::OriginId, ty: Ty, f: &Func, name: &str, interface: &Func) {
        let named = |f: &Func| OriginTable::global().declaration_name(f.literal.expr.origin);
        let made = interface.within.as_ref().and_then(|made| named(&made.f));
        let wanted = named(interface).or(made).map_or_else(|| "its interface".to_string(), |n| n.to_string());
        let msg = Msg::new().ty(ty.to_string()).text(" does not meet the bound ").code(wanted.clone());
        let note = Msg::new()
            .text("the parameter ")
            .code(format!("${name}"))
            .text(" of ")
            .code(named(f).map_or_else(|| "the function".to_string(), |n| n.to_string()));
        let help = Msg::new().text("give the type an impl of ").code(wanted).text(", or pass another");
        let d = Diagnostic::error(errors::TYPES_NO_IMPL, msg)
            .at(at, None)
            .child(ChildKind::Note, note.text(" requires it"))
            .child(ChildKind::Help, help);
        self.diag.error(d);
    }
}
