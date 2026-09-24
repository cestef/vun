//! Attributes as data (spec 02, section 5): an attribute the compiler acts on is a call of a
//! `lang` primitive, found by its value, never by its name.

use overt_hir::tree::{Arg, Entry, ExprId, ExprKind};

use super::Env;
use crate::Primitive;
use crate::ty::Value;

impl Env<'_, '_> {
    /// The attributes written on the declaration whose value is the literal `literal`.
    #[must_use]
    pub fn attrs_of(&self, literal: ExprId) -> Vec<ExprId> {
        let hir = &self.parsed.hir;
        hir.decls
            .iter()
            .find(|(_, d)| d.value == Some(literal))
            .map(|(_, d)| d.attrs.clone())
            .unwrap_or_default()
    }

    /// The arguments of the attribute `primitive` among `attrs`, evaluated: `#[layout.bits(u32)]`
    /// gives `[u32]`, `#[layout.c]` gives none; `None` when no attribute is it, or when an
    /// argument has no value here.
    #[must_use]
    pub fn attribute(&self, attrs: &[ExprId], primitive: Primitive) -> Option<Vec<Value>> {
        let hir = &self.parsed.hir;
        attrs.iter().find_map(|&attr| {
            let (callee, args) = match &hir.exprs[attr].kind {
                ExprKind::Call { callee, args } => (*callee, args.as_slice()),
                _ => (attr, [].as_slice()),
            };
            if self.eval(callee).ok()? != Value::Primitive(primitive) {
                return None;
            }
            args.iter()
                .map(|arg| match arg {
                    Entry::One(Arg { name: None, value }) => self.eval(*value).ok(),
                    _ => None,
                })
                .collect()
        })
    }
}
