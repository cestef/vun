//! Attributes as data (spec 02, section 5): an attribute the compiler acts on is a call of a
//! `lang` primitive, found by its value, never by its name.

use vun_hir::tree::{Arg, Entry, ExprId, ExprKind};

use super::Env;
use crate::Primitive;
use crate::ty::{Marker, Signature, Value};

impl Env<'_, '_> {
    /// The attributes of the literal `literal`: written on it (`#[kind.linear] struct { }`) or on
    /// the declaration whose value it is.
    #[must_use]
    pub fn attrs_of(&self, literal: ExprId) -> Vec<ExprId> {
        let hir = &self.parsed.hir;
        let mut attrs = Vec::new();
        // Up through the attributes and marker prefixes around it, as in
        // `#[panic.never] of := unsafe (...) { ... }`, to the declaration it is the value of.
        let mut value = literal;
        while let Some((id, e)) = hir.exprs.iter().find(|(_, e)| match &e.kind {
            ExprKind::Attributed { inner, .. } => *inner == value,
            ExprKind::Prefix { op, operand, .. } => *operand == value && Marker::of_prefix(*op).is_some(),
            _ => false,
        }) {
            if let ExprKind::Attributed { attrs: on, .. } = &e.kind {
                attrs.extend(on);
            }
            value = id;
        }
        if let Some((_, decl)) = hir.decls.iter().find(|(_, d)| d.value == Some(value)) {
            attrs.extend(&decl.attrs);
        }
        attrs
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

    /// The count the attribute `primitive` gives among `attrs`, as `n` of `#[stack.max(n)]`.
    #[must_use]
    pub fn count(&self, attrs: &[ExprId], primitive: Primitive) -> Option<u64> {
        let args = self.attribute(attrs, primitive)?;
        let [Value::Int(n)] = args.as_slice() else { return None };
        u64::try_from(n.to_i128()?).ok()
    }

    /// The markers among `attrs`: those the table `fn_markers` names, with their arguments.
    #[must_use]
    pub fn markers(&self, attrs: &[ExprId]) -> Vec<Marker> {
        let mut markers = Vec::new();
        for (item, _) in Marker::kinds() {
            if let Some(args) = self.attribute(attrs, item) {
                Marker::insert(&mut markers, Marker { item, args });
            }
        }
        markers
    }

    /// `value` marked by the markers among `attrs` when it is a function type, as in
    /// `#[panic.never] (u8) -> u8`; any other value as it is.
    #[must_use]
    pub fn marked_by(&self, value: Value, attrs: &[ExprId]) -> Value {
        match value.as_type().and_then(|t| t.signature()) {
            Some(signature) => {
                Value::Type(self.markers(attrs).into_iter().fold(signature, Signature::with).ty())
            }
            None => value,
        }
    }
}
