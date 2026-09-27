//! Checking a file (spec 01, 02 and 04): each namespace member's value against its annotation,
//! each function body, wherever the types are known, and where restricted names are visible.

mod c_layout;
mod frozen;
mod holes;
mod impls;
mod layout;
mod link;
mod members;
mod placed;
mod visible;

use std::sync::Arc;

use vun_diag::DiagCtxt;
use vun_hir::db::FileRef;
use vun_hir::tree::{DeclId, ExprId, ExprKind, Hir, Member, NsId, each_entry};
use vun_hir::visit::{self, Visitor};
use vun_query::{Cx, Query};

use crate::decl::{DeclValue, TypedDecl};
use crate::env::{Env, Unevaluated};
use crate::eval::Instantiation;
use crate::item::DeclRef;
use crate::item::ExprRef;
use crate::typeck::Typed;
use crate::visible::Home;

/// What checking a file reports; `None` for a file the driver did not read.
pub struct Checked;

impl Query for Checked {
    const NAME: &'static str = "checked";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        let env = Env::of(cx, key)?;
        let mut checker =
            Checker { env: &env, diag: DiagCtxt::new(), homes: vec![env.home()], interfaces: 0 };
        if let Some(root) = env.parsed.hir.root {
            checker.namespace(&env.parsed.hir, root);
        }
        checker.placement(&env.parsed.hir);
        checker.impls();
        checker.exports();
        Some(Arc::new(checker.diag))
    }
}

impl Checker<'_, '_, '_> {
    /// Computes the value of the member declaration `decl`, as statics are at compile time
    /// (spec 00, guarantee 1), reporting a trap that fires.
    fn evaluate(&mut self, hir: &Hir, decl: DeclId) {
        let Some(value) = hir.decls[decl].value else { return };
        let decl = DeclRef::new(self.env.file.clone(), hir, decl);
        if let Err(Unevaluated::Trapped(trap)) = self.env.cx.get::<DeclValue>(&decl) {
            self.diag.error(trap.report(hir.exprs[value].origin));
        }
    }
}

/// Walks a whole file, checking what is typed apart: namespace members and function bodies.
struct Checker<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    diag: DiagCtxt,
    /// The namespaces around the members visited, innermost last.
    homes: Vec<Home>,
    /// How many `lang.interface(...)` calls enclose what is visited: requirements may be `_`.
    interfaces: usize,
}

impl Visitor for Checker<'_, '_, '_> {
    /// A spread's path starts at a package, which the check of names resolves.
    fn spread(&mut self, _: &Hir, _: &vun_hir::tree::Spread) {}

    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        each_entry(&hir.namespaces[ns].members, &mut |m| {
            self.visibility(hir, m);
            if let Member::Decl(d) = m {
                self.evaluate(hir, *d);
                self.hole(hir, *d);
                self.frozen(hir, *d);
                self.layout(hir, *d);
                self.c_layout(hir, *d);
                let key = (DeclRef::new(self.env.file.clone(), hir, *d), None, Vec::new());
                if let Some(typed) = self.env.cx.get::<TypedDecl>(&key) {
                    self.diag.absorb(&typed.diag);
                }
            }
        });
        visit::walk_namespace(self, hir, ns);
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        self.member(hir, expr);
        if let ExprKind::Fn { .. } = hir.exprs[expr].kind {
            let f = ExprRef::new(self.env.file.clone(), hir, expr);
            if let Some(typed) = self.env.cx.get::<Typed>(&Instantiation::plain(f)) {
                self.diag.absorb(&typed.diag);
            }
        }
        if let ExprKind::Call { callee, .. } = hir.exprs[expr].kind
            && self.is_interface(callee)
        {
            self.interfaces += 1;
            visit::walk_expr(self, hir, expr);
            self.interfaces -= 1;
            return;
        }
        if let ExprKind::Struct(_) = hir.exprs[expr].kind {
            self.homes.push(Home::Struct(ExprRef::new(self.env.file.clone(), hir, expr)));
            visit::walk_expr(self, hir, expr);
            self.homes.pop();
            return;
        }
        visit::walk_expr(self, hir, expr);
    }
}
