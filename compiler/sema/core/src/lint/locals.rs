//! The local bindings and parameters lints look at in a declaration's value.

use std::collections::HashSet;

use vun_hir::tree::{BinderId, ExprId, ExprKind, Hir, Joined, List, NsId, Param, PatId, PatKind, each_entry};
use vun_hir::visit::{self, Visitor};

/// The local bindings and parameters in an expression, outside the bodies of types it declares:
/// each binder, and whether a function literal's parameter binds it. A function type's
/// parameters name no binding, and the binders of an or-pattern's alternatives after the first
/// are the first's, or names nothing can read.
#[derive(Default)]
pub(super) struct Locals {
    bound: Vec<BinderId>,
    params: HashSet<BinderId>,
    /// Binders that are no local of their own.
    skipped: HashSet<BinderId>,
}

impl Locals {
    /// Each binder bound, in order, and whether it is a parameter.
    pub(super) fn found(self) -> Vec<(BinderId, bool)> {
        let Self { bound, params, skipped } = self;
        bound.into_iter().filter(|b| !skipped.contains(b)).map(|b| (b, params.contains(&b))).collect()
    }

    /// The binders of the parameters `params`.
    fn binders(hir: &Hir, params: &List<Param>) -> Vec<BinderId> {
        let mut binders = Vec::new();
        each_entry(params, &mut |p: &Param| {
            if let Some(pat) = p.pat {
                hir.binders(pat, &mut binders);
            }
        });
        binders
    }
}

impl Visitor for Locals {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        match &hir.exprs[expr].kind {
            ExprKind::Fn { params, .. } => self.params.extend(Self::binders(hir, params)),
            ExprKind::FnType { params, .. } => self.skipped.extend(Self::binders(hir, params)),
            _ => {}
        }
        visit::walk_expr(self, hir, expr);
    }

    fn pat(&mut self, hir: &Hir, pat: PatId) {
        match &hir.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) | PatKind::At { binder: b, .. } => self.bound.push(*b),
            PatKind::Or(alternatives) => {
                let others = hir.joined(alternatives).into_iter().filter_map(|j| match j {
                    Joined::Paired { other, .. } | Joined::Extra { other } => Some(other),
                    Joined::Missing { .. } => None,
                });
                self.skipped.extend(others);
            }
            _ => {}
        }
        visit::walk_pat(self, hir, pat);
    }

    /// A namespace inside a value is a type's, whose members are declarations of their own.
    fn namespace(&mut self, _: &Hir, _: NsId) {}
}
