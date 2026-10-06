//! The freeze rule (spec 03, section 6): a type's member set is fixed once the expression
//! defining it is evaluated, so a member of a `struct` literal cannot reflect on the type that
//! literal makes: `meta.frozen` for `fields`, `size` or `align` of it.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{Arg, DeclId, Entry, ExprId, ExprKind, Hir};
use vun_hir::visit::{self, Visitor};
use vun_primitives::Primitive;
use vun_primitives::meta::Meta;
use vun_sema_ty::{Nominal, TyKind, Value};

use super::Checker;
use vun_sema::visible::Home;

impl Checker<'_, '_, '_> {
    /// Reports each reflection on the enclosing `struct` literal's type in the value of `decl`.
    pub(super) fn frozen(&mut self, hir: &Hir, decl: DeclId) {
        let (Some(Home::Struct(literal)), Some(value)) = (self.homes.last(), hir.decls[decl].value) else {
            return;
        };
        let mut calls = Calls(Vec::new());
        calls.expr(hir, value);
        for (callee, arg) in calls.0 {
            let reflects = matches!(
                self.env.eval(callee),
                Ok(Value::Primitive(Primitive::Meta(Meta::fields | Meta::size | Meta::align)))
            );
            let own = self.env.eval(arg).ok().and_then(|v| v.as_type()).is_some_and(|ty| {
                matches!(&*ty, TyKind::Nominal(Nominal { literal: l, .. }) if l.expr.id == literal.expr.id && l.file == literal.file)
            });
            if reflects && own {
                let msg = Msg::new().text("this reads the members of the type its body declares");
                let d = Diagnostic::error(errors::META_FROZEN, msg)
                    .at(hir.exprs[callee].origin, None)
                    .note("a type's members are known only once its whole body is")
                    .help("read them outside the type's body, such as in a function it declares");
                self.diag.error(d);
            }
        }
    }
}

/// The calls in an expression with one argument, by callee and argument, outside the bodies of
/// the functions it makes, which run once the type is made.
struct Calls(Vec<(ExprId, ExprId)>);

impl Visitor for Calls {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if matches!(hir.exprs[expr].kind, ExprKind::Fn { .. }) {
            return;
        }
        if let ExprKind::Call { callee, args } = &hir.exprs[expr].kind
            && let [Entry::One(Arg { name: None, value })] = args.as_slice()
        {
            self.0.push((*callee, *value));
        }
        visit::walk_expr(self, hir, expr);
    }
}
