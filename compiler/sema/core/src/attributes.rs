//! The attributes of an item, read one way (spec 02, section 5): the environment evaluating them,
//! and where they are written. Markers among them (spec 07, section 4) are read as markers, the
//! one form the function type carries too.

use vun_hir::tree::ExprId;
use vun_intern::Symbol;
use vun_primitives::Primitive;
use vun_primitives::abi::Abi;
use vun_query::Cx;
use vun_sema_ty::call::Func;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Marker, Nominal, Value};

use crate::env::Env;

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

/// An item whose attributes are read: a function, a type or a declaration.
pub trait Attributed {
    /// The attributes of this item: those around its literal and on the declaration it is the
    /// value of, evaluated where the literal was (for a type, in the call that made it).
    #[must_use]
    fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>>;
}

impl Attributed for Func {
    fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::defining(cx, &self.literal, self.within.as_deref())?;
        let exprs = env.attrs_of(self.literal.expr.id);
        let markers = env.literal_markers(self.literal.expr.id);
        Some(Attributes { env, exprs, markers })
    }
}

impl Attributed for Nominal {
    fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::defining(cx, &self.literal, self.instance.as_ref())?;
        let exprs = env.attrs_of(self.literal.expr.id);
        let markers = env.markers(&exprs);
        Some(Attributes { env, exprs, markers })
    }
}

impl Attributed for DeclRef {
    fn attributes<'a, 'db>(&self, cx: &'a Cx<'db>) -> Option<Attributes<'a, 'db>> {
        let env = Env::of(cx, &self.file)?;
        let exprs = env.parsed.hir.decls[self.decl.id].attrs.clone();
        let markers = env.markers(&exprs);
        Some(Attributes { env, exprs, markers })
    }
}
