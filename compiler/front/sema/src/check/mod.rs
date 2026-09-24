//! Checking a file (spec 01, 02 and 04): each namespace member's value against its annotation,
//! each function body, wherever the types are known, and where restricted names are visible.

mod visible;

use std::sync::Arc;

use overt_diag::DiagCtxt;
use overt_hir::db::FileRef;
use overt_hir::tree::{ExprId, ExprKind, Hir, Member, NsId, each_entry};
use overt_hir::visit::{self, Visitor};
use overt_query::{Cx, Query};

use crate::env::Env;
use crate::item::ExprRef;
use crate::typeck::{Stage, Typed, Typer};
use crate::visible::Home;

/// What checking a file reports; `None` for a file the driver did not read.
pub struct Checked;

impl Query for Checked {
    const NAME: &'static str = "checked";
    type Key = FileRef;
    type Value = Option<Arc<DiagCtxt>>;

    fn compute(cx: &Cx<'_>, key: &FileRef) -> Self::Value {
        let env = Env::of(cx, key)?;
        let mut checker = Checker { env: &env, diag: DiagCtxt::new(), homes: vec![env.home()] };
        if let Some(root) = env.parsed.hir.root {
            checker.namespace(&env.parsed.hir, root);
        }
        Some(Arc::new(checker.diag))
    }
}

/// Walks a whole file, checking what is typed apart: namespace members and function bodies.
struct Checker<'e, 'a, 'db> {
    env: &'e Env<'a, 'db>,
    diag: DiagCtxt,
    /// The namespaces around the members visited, innermost last.
    homes: Vec<Home>,
}

impl Visitor for Checker<'_, '_, '_> {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        each_entry(&hir.namespaces[ns].members, &mut |m| {
            self.visibility(hir, m);
            if let Member::Decl(d) = m {
                let mut typer = Typer::new(self.env, Stage::Compile);
                typer.decl(hir, *d);
                self.diag.absorb(&typer.diag);
            }
        });
        visit::walk_namespace(self, hir, ns);
    }

    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if let ExprKind::Fn { .. } = hir.exprs[expr].kind {
            let f = ExprRef::new(self.env.file.clone(), hir, expr);
            if let Some(typed) = self.env.cx.get::<Typed>(&f) {
                self.diag.absorb(&typed.diag);
            }
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
