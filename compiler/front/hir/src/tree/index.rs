//! What each node of a file relates to, found in one walk rather than by scanning the file at
//! every question: the declaration and namespace of each binder, the spread selecting it, the
//! `struct` literal of each namespace, the closure of each function literal, the function
//! literal around each expression, and what wraps an expression as attributes and prefixes do.

use vun_index::IndexVec;

use super::binders::each_entry;
use super::{BinderId, DeclId, ExprId, ExprKind, Hir, Member, NsId};
use crate::visit::{self, Visitor};

pub(crate) struct Index {
    /// The declaration binding each binder, among members or in a block.
    pub(crate) decl: IndexVec<BinderId, Option<DeclId>>,
    /// The namespace and member declaration binding each binder; `None` for a local.
    pub(crate) member: IndexVec<BinderId, Option<(NsId, DeclId)>>,
    /// The path of the spread selecting each binder, as `..std.mem.{ Own }` selects `Own`.
    pub(crate) spread: IndexVec<BinderId, Option<ExprId>>,
    /// The `struct` literal whose members each namespace holds.
    pub(crate) literal: IndexVec<NsId, Option<ExprId>>,
    /// The closure each function literal is the function of.
    pub(crate) closure: IndexVec<ExprId, Option<ExprId>>,
    /// The innermost function literal whose body holds each expression.
    pub(crate) function: IndexVec<ExprId, Option<ExprId>>,
    /// The attributes `#[...] e` or the prefix operator applied to each expression.
    pub(crate) wrapper: IndexVec<ExprId, Option<ExprId>>,
    /// The declaration each expression is the value of.
    pub(crate) valued: IndexVec<ExprId, Option<DeclId>>,
}

impl Index {
    pub(crate) fn of(hir: &Hir) -> Self {
        let mut index = Self {
            decl: IndexVec::filled(None, hir.binders.len()),
            member: IndexVec::filled(None, hir.binders.len()),
            spread: IndexVec::filled(None, hir.binders.len()),
            literal: IndexVec::filled(None, hir.namespaces.len()),
            closure: IndexVec::filled(None, hir.exprs.len()),
            function: IndexVec::filled(None, hir.exprs.len()),
            wrapper: IndexVec::filled(None, hir.exprs.len()),
            valued: IndexVec::filled(None, hir.exprs.len()),
        };
        let mut binders = Vec::new();
        for (decl, d) in hir.decls.iter() {
            if let Some(value) = d.value {
                index.valued[value].get_or_insert(decl);
            }
            binders.clear();
            hir.binders(d.pat, &mut binders);
            for &b in &binders {
                index.decl[b].get_or_insert(decl);
            }
        }
        for (ns, namespace) in hir.namespaces.iter() {
            each_entry(&namespace.members, &mut |member| match member {
                Member::Decl(decl) => {
                    binders.clear();
                    hir.binders(hir.decls[*decl].pat, &mut binders);
                    for &b in &binders {
                        index.member[b].get_or_insert((ns, *decl));
                    }
                }
                Member::Spread(spread) => {
                    for &b in spread.select.iter().flatten() {
                        index.spread[b] = Some(spread.path);
                    }
                }
                Member::Field(_) => {}
            });
        }
        for (e, expr) in hir.exprs.iter() {
            match expr.kind {
                ExprKind::Struct(ns) => {
                    index.literal[ns].get_or_insert(e);
                }
                ExprKind::Closure { func, .. } => {
                    index.closure[func].get_or_insert(e);
                }
                ExprKind::Attributed { inner: wrapped, .. } | ExprKind::Prefix { operand: wrapped, .. } => {
                    index.wrapper[wrapped].get_or_insert(e);
                }
                _ => {}
            }
        }
        if let Some(root) = hir.root {
            Functions { around: Vec::new(), function: &mut index.function }.namespace(hir, root);
        }
        index
    }
}

/// Walks the tree, recording for each expression the function literal around it.
struct Functions<'i> {
    around: Vec<ExprId>,
    function: &'i mut IndexVec<ExprId, Option<ExprId>>,
}

impl Visitor for Functions<'_> {
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        if self.function[expr].is_none() {
            self.function[expr] = self.around.last().copied();
        }
        let function = matches!(hir.exprs[expr].kind, ExprKind::Fn { .. });
        if function {
            self.around.push(expr);
        }
        visit::walk_expr(self, hir, expr);
        if function {
            self.around.pop();
        }
    }
}
