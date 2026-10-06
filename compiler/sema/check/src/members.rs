//! Members of namespaces (spec 01, section 7): a path naming a member its namespace lacks is
//! reported wherever it is written, in a body, a signature or a type, typed or not.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::db::Exports;
use vun_hir::tree::{ExprId, ExprKind, Hir, Name};
use vun_sema_ty::Value;
use vun_sema_ty::item::Declared;
use vun_suggest::closest;

use super::Checker;

impl Checker<'_, '_, '_> {
    /// Reports the member `expr` names when it is written on a namespace whose members are all
    /// known and it is none of them.
    pub(super) fn member(&mut self, hir: &Hir, expr: ExprId) {
        let ExprKind::Member { object, name: Name::Plain(name) } = hir.exprs[expr].kind else { return };
        // Only a path of names can be a namespace; anything else, a call included, is not run.
        let Some((_, path)) = hir.path(object) else { return };
        let Ok(Value::Namespace(ns)) = self.env.eval(object) else { return };
        let cx = self.env.cx;
        if cx.get::<Declared>(&(ns.clone(), name)).is_some() || !Declared::complete(cx, &ns) {
            return;
        }
        let msg =
            Msg::new().path(path.iter().map(|s| s.as_str())).text(" has no member ").code(name.as_str());
        let mut d = Diagnostic::error(errors::NAMES_UNRESOLVED, msg).at(hir.exprs[expr].origin, None);
        let exports = cx.get::<Exports>(&ns);
        if let Some(near) = closest(name.as_str(), exports.iter().map(|s| s.as_str())) {
            d = d.help(Msg::did_you_mean(near));
        }
        self.diag.error(d);
    }
}
