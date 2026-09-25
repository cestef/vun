//! Where attributes are written (spec 02, section 5): an attribute of type `lang.Attribute(on)`
//! only at one of the places `on`; any other attribute is plain data, written anywhere. The
//! sites are the arenas of the HIR: declarations, fields, parameters and attributed expressions.

use vun_diag::{Diagnostic, Msg, errors};
use vun_hir::tree::{Entry, ExprId, ExprKind, Hir, Member, Param, each_entry};
use vun_intern::Symbol;
use vun_items::Item;
use vun_items::lang::Place;

use super::Checker;
use crate::ty::{Marker, Nominal, TyKind, Value};
use crate::typeck::{Stage, Typer};

impl Checker<'_, '_, '_> {
    /// Reports each attribute of the file written at none of the places its type lists.
    pub(super) fn placement(&mut self, hir: &Hir) {
        for (_, d) in hir.decls.iter() {
            let mut here = vec![Place::decls];
            here.extend(d.value.and_then(|v| Self::place_of(hir, v)));
            // `= _` of a function type provides a function.
            let hole = d.value.is_some_and(|v| matches!(hir.exprs[v].kind, ExprKind::Hole));
            if hole && d.ty.and_then(|t| Self::place_of(hir, t)) == Some(Place::functions) {
                here.push(Place::functions);
            }
            self.placed(hir, &d.attrs, &here);
        }
        for (_, ns) in hir.namespaces.iter() {
            each_entry(&ns.members, &mut |member| {
                if let Member::Field(field) = member {
                    self.placed(hir, &field.attrs, &[Place::fields]);
                }
            });
        }
        for (_, e) in hir.exprs.iter() {
            match &e.kind {
                ExprKind::Fn { params, .. } => {
                    for param in params {
                        if let Entry::One(Param { attrs, .. }) = param {
                            self.placed(hir, attrs, &[Place::params]);
                        }
                    }
                }
                ExprKind::Attributed { attrs, inner } => {
                    let here: Vec<Item> = Self::place_of(hir, *inner).into_iter().collect();
                    self.placed(hir, attrs, &here);
                }
                _ => {}
            }
        }
    }

    /// The place an expression is, through attributes and marker prefixes around it: a
    /// function literal or type is a function, a `struct` or `enum` literal a type.
    fn place_of(hir: &Hir, mut expr: ExprId) -> Option<Item> {
        loop {
            match &hir.exprs[expr].kind {
                ExprKind::Attributed { inner, .. } => expr = *inner,
                ExprKind::Prefix { op, operand, .. } if Marker::of_prefix(*op).is_some() => expr = *operand,
                ExprKind::Fn { .. } | ExprKind::FnType { .. } => return Some(Place::functions),
                ExprKind::Struct(_) | ExprKind::Enum { .. } => return Some(Place::types),
                _ => return None,
            }
        }
    }

    /// Reports each of `attrs` whose type lists places none of which is `here`.
    fn placed(&mut self, hir: &Hir, attrs: &[ExprId], here: &[Item]) {
        for &attr in attrs {
            let Some(on) = self.places(attr) else { continue };
            if on.iter().any(|place| here.iter().any(|h| h.symbol() == *place)) {
                continue;
            }
            let msg =
                Msg::new().text("this attribute applies only to ").code_list(on.iter().map(|p| p.as_str()));
            self.diag.error(Diagnostic::error(errors::ATTR_MISPLACED, msg).at(hir.exprs[attr].origin, None));
        }
    }

    /// The places the attribute at `attr` applies to, when its type is `lang.Attribute(on)`.
    fn places(&self, attr: ExprId) -> Option<Vec<Symbol>> {
        let ty = Typer::new(self.env, Stage::Compile).typed(attr)?.ty;
        let TyKind::Nominal(Nominal { instance: Some(made), .. }) = &*ty else { return None };
        if Value::Fn(made.f.clone()) != self.env.item(vun_items::lang::Attribute).ok()? {
            return None;
        }
        let [Value::Array(on)] = made.args.as_slice() else { return None };
        on.iter()
            .map(|place| match place {
                Value::Variant { name, .. } => Some(*name),
                _ => None,
            })
            .collect()
    }
}
