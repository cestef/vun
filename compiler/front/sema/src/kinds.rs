//! Kinds (spec 05, section 1): whether values of a type are duplicated by `=` or must be consumed
//! exactly once, and whether they hold a borrow, computed from its parts. `#[kind.linear]` makes a
//! type linear, and so are coroutine frames; everything built only from copy parts is copy.
//! Borrows are copy here: that a `&mut` is exclusive is the borrow rules' concern (section 8),
//! not a kind. A borrow type is scoped, and so is anything with a scoped part; a raw pointer is
//! free whatever it points to. A frame is free until its parameters are read here.

use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::Env;
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
}

impl Kind {
    const COPY_FREE: Self = Self { duplication: Duplication::Copy, region: Region::Free };

    /// A value made of parts of these kinds: linear if one is, scoped if one is.
    fn of_parts(parts: impl IntoIterator<Item = Self>) -> Self {
        parts.into_iter().fold(Self::COPY_FREE, |kind, part| Self {
            duplication: if part.duplication == Duplication::Linear {
                part.duplication
            } else {
                kind.duplication
            },
            region: if part.region == Region::Scoped { part.region } else { kind.region },
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
        if ty.borrowing().is_some() {
            return Kind { duplication: Duplication::Copy, region: Region::Scoped };
        }
        match &**ty {
            TyKind::Primitive(Primitive::Mem(Mem::Array | Mem::Slice | Mem::Cell), args) => {
                Kind::of_parts(args.iter().filter_map(Value::as_type).map(|t| of(&t)))
            }
            TyKind::Primitive(..) => Kind::COPY_FREE,
            TyKind::Tuple(items) => Kind::of_parts(items.iter().map(of)),
            TyKind::Closure(Closure { captures, .. }) => Kind::of_parts(captures.iter().map(|(_, t)| of(t))),
            TyKind::Frame(_) => Kind { duplication: Duplication::Linear, region: Region::Free },
            TyKind::Nominal(nominal) => Self::nominal(cx, *ty, nominal),
        }
    }

    /// A type whose kind depends on itself, such as a recursive enum, is as its other parts make it.
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        Kind::COPY_FREE
    }
}

impl Kinds {
    fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Kind {
        let marked = Env::defining(cx, &nominal.literal, nominal.instance.as_ref()).is_some_and(|env| {
            let attrs = env.attrs_of(nominal.literal.expr.id);
            env.attribute(&attrs, Primitive::Kind(KindAttr::linear)).is_some()
        });
        let Some(members) = cx.get::<Members>(&ty) else {
            return Kind {
                duplication: if marked { Duplication::Linear } else { Duplication::Copy },
                ..Kind::COPY_FREE
            };
        };
        let fields = members.fields.iter().filter_map(|f| f.ty.as_ref().ok());
        let payloads = members.variants.iter().flat_map(|v| v.payload.iter().filter_map(|p| p.as_ref().ok()));
        let kind = Kind::of_parts(fields.chain(payloads).map(|t| cx.get::<Self>(t)));
        if marked { Kind { duplication: Duplication::Linear, ..kind } } else { kind }
    }
}
