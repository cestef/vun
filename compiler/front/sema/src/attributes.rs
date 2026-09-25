//! The attributes of an item, read one way (spec 02, section 5): the environment evaluating them,
//! and where they are written. Markers among them (spec 07, section 4) are read as markers, the
//! one form the function type carries too.

use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_query::Cx;

use crate::Primitive;
use crate::env::Env;
use crate::eval::Func;
use crate::item::DeclRef;
use crate::primitives::abi::Abi;
use crate::ty::{Marker, Nominal, Value};

/// The attributes written on one item.
pub struct Attributes<'a, 'db> {
    env: Env<'a, 'db>,
    exprs: Vec<ExprId>,
    markers: Vec<Marker>,
}

impl Attributes<'_, '_> {
    /// The arguments of the attribute `attr`, if it is written here.
    #[must_use]
    pub fn get(&self, attr: Primitive) -> Option<Vec<Value>> {
        self.env.attribute(&self.exprs, attr)
    }

    /// The count `attr` gives, as `n` of `#[comptime.fuel(n)]`.
    #[must_use]
    pub fn count(&self, attr: Primitive) -> Option<u64> {
        self.env.count(&self.exprs, attr)
    }

    /// The text `attr` gives, as the symbol of `#[link.export(s)]`.
    #[must_use]
    pub fn text(&self, attr: Primitive) -> Option<Symbol> {
        Some(Symbol::intern(&self.get(attr)?.first()?.text()?))
    }

    /// The markers among them, and those the prefixes around a function put.
    #[must_use]
    pub fn markers(&self) -> &[Marker] {
        &self.markers
    }

    /// The calling convention its markers name (`#[abi.c]`), if any.
    #[must_use]
    pub fn abi(&self) -> Option<Abi> {
        Marker::abi(&self.markers)
    }

    /// The most bytes of stack a call takes, `#[stack.max(n)]`, if bound.
    #[must_use]
    pub fn stack_max(&self) -> Option<u64> {
        Marker::stack_max(&self.markers)
    }
}

impl Func {
    /// The attributes of this function: those around its literal and on the declaration it is
    /// the value of, evaluated where the literal was.
    #[must_use]
    pub fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::defining(cx, &self.literal, self.within.as_deref())?;
        let exprs = env.attrs_of(self.literal.expr.id);
        let markers = env.literal_markers(self.literal.expr.id);
        Some(Attributes { env, exprs, markers })
    }
}

impl Nominal {
    /// The attributes of this type: those around its `struct` or `enum` literal and on the
    /// declaration it is the value of, evaluated in the call that made it.
    #[must_use]
    pub fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::defining(cx, &self.literal, self.instance.as_ref())?;
        let exprs = env.attrs_of(self.literal.expr.id);
        let markers = env.markers(&exprs);
        Some(Attributes { env, exprs, markers })
    }
}

impl DeclRef {
    /// The attributes of this declaration.
    #[must_use]
    pub fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::of(cx, &self.file)?;
        let exprs = env.parsed.hir.decls[self.decl.id].attrs.clone();
        let markers = env.markers(&exprs);
        Some(Attributes { env, exprs, markers })
    }
}
