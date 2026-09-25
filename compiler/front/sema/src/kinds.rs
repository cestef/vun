//! Kinds (spec 05, section 1): whether values of a type are duplicated by `=` or must be consumed
//! exactly once, and whether they hold a borrow, computed from its parts. `#[kind.linear]` makes a
//! type linear, and so are coroutine frames; everything built only from copy parts is copy.
//! Borrows are copy here: that a `&mut` is exclusive is the borrow rules' concern (section 8),
//! not a kind. A borrow type is scoped, and so is anything with a scoped part; a raw pointer is
//! free whatever it points to. A frame is free until its parameters are read here. A type marked
//! `#[kind.free]` is free, and holding a scoped value in it, even through a type argument, is an
//! error its kind records.

use vun_desugar::Extent;
use vun_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::member::Members;
use crate::primitives::kind::Kind as KindAttr;
use crate::primitives::mem::Mem;
use crate::ty::{Closure, Nominal, Ty, TyKind, Value};

/// Whether values are duplicated or consumed once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Duplication {
    Copy,
    Linear,
}

/// Whether values hold a borrow, which ties them to the scope it was taken in (section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    Free,
    Scoped,
}

/// The properties of a type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Kind {
    pub duplication: Duplication,
    pub region: Region,
    /// Whether some type in it marked `#[kind.free]` holds a scoped value, in a part or a type
    /// argument (`borrow.in_free_value`).
    pub misplaced: bool,
    /// Whether a part is a cell (`lang.mem.Cell`), which changes through shared borrows: its
    /// storage is written even where it is bound immutably.
    pub interior: bool,
}

impl Kind {
    const COPY_FREE: Self =
        Self { duplication: Duplication::Copy, region: Region::Free, misplaced: false, interior: false };

    /// A value made of parts of these kinds: linear if one is, scoped if one is, misplaced if one
    /// is, with a cell if one has.
    fn of_parts(parts: impl IntoIterator<Item = Self>) -> Self {
        parts.into_iter().fold(Self::COPY_FREE, |kind, part| Self {
            duplication: if part.is_linear() { part.duplication } else { kind.duplication },
            region: if part.is_scoped() { part.region } else { kind.region },
            misplaced: kind.misplaced || part.misplaced,
            interior: kind.interior || part.interior,
        })
    }

    #[must_use]
    pub fn is_linear(self) -> bool {
        self.duplication == Duplication::Linear
    }

    #[must_use]
    pub fn is_scoped(self) -> bool {
        self.region == Region::Scoped
    }
}

/// The kind of a type; copy and free for a type whose parts are not known, so that checks built
/// on it stay silent.
pub struct Kinds;

impl Query for Kinds {
    const NAME: &'static str = "kinds";
    type Key = Ty;
    type Value = Kind;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let of = |ty: &Ty| cx.get::<Self>(ty);
        // A borrow is scoped, but for a `&static` one, which lives as long as the program.
        if ty.borrowing().is_some() {
            let region = if ty.extent() == Some(Extent::Static) { Region::Free } else { Region::Scoped };
            return Kind { region, ..Kind::COPY_FREE };
        }
        match &**ty {
            TyKind::Primitive(Primitive::Mem(m @ (Mem::Array | Mem::Slice | Mem::Cell)), args) => Kind {
                interior: *m == Mem::Cell,
                ..Kind::of_parts(args.iter().filter_map(Value::as_type).map(|t| of(&t)))
            },
            // A constrained `$` parameter is unknown until an instantiation binds it, so checks
            // built on it stay silent.
            TyKind::Primitive(..) | TyKind::Param(_) => Kind::COPY_FREE,
            TyKind::Tuple(items) => Kind::of_parts(items.iter().map(of)),
            TyKind::Record(fields) => Kind::of_parts(fields.iter().map(|(_, t)| of(t))),
            TyKind::Closure(Closure { captures, .. }) => Kind::of_parts(captures.iter().map(|(_, t)| of(t))),
            TyKind::Frame(_) => Kind { duplication: Duplication::Linear, ..Kind::COPY_FREE },
            TyKind::Nominal(nominal) => Self::nominal(cx, *ty, nominal),
        }
    }

    /// A type whose kind depends on itself, such as a recursive enum, is as its other parts make it.
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        Kind::COPY_FREE
    }
}

impl Kinds {
    /// A struct or enum: as its parts make it, then as `#[kind.linear]` and `#[kind.free]` on its
    /// literal mark it.
    fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Kind {
        let marked = |attr| Self::marked(cx, nominal, Primitive::Kind(attr));
        let parts = cx.get::<Members>(&ty).map_or(Kind::COPY_FREE, |members| {
            let fields = members.fields.iter().filter_map(|f| f.ty.as_ref().ok());
            let payloads =
                members.variants.iter().flat_map(|v| v.payload.iter().filter_map(|p| p.as_ref().ok()));
            Kind::of_parts(fields.chain(payloads).map(|t| cx.get::<Self>(t)))
        });
        let mut kind = parts;
        if marked(KindAttr::linear) {
            kind.duplication = Duplication::Linear;
        }
        if marked(KindAttr::free) {
            let args = nominal.instance.iter().flat_map(|i| i.args.iter().filter_map(Value::as_type));
            let args = Kind::of_parts(args.map(|t| cx.get::<Self>(&t)));
            kind.misplaced |= parts.is_scoped() || args.is_scoped() || args.misplaced;
            kind.region = Region::Free;
        }
        kind
    }

    /// Whether `attr` marks the literal of `nominal`.
    fn marked(cx: &Cx<'_>, nominal: &Nominal, attr: Primitive) -> bool {
        nominal.attributes(cx).is_some_and(|a| a.get(attr).is_some())
    }
}
