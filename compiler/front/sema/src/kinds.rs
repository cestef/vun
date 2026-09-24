//! Kinds (spec 05, section 1): whether values of a type are duplicated by `=` or must be consumed
//! exactly once, computed from its parts. `#[kind.linear]` makes a type linear, and so are
//! coroutine frames; everything built only from copy parts is copy. Borrows are copy here: that
//! a `&mut` is exclusive is the borrow rules' concern (section 7), not a kind.

use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::Env;
use crate::member::Members;
use crate::primitives::kind::Kind;
use crate::primitives::mem::Mem;
use crate::ty::{Closure, Nominal, Ty, TyKind, Value};

/// Whether values are duplicated or consumed once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Duplication {
    Copy,
    Linear,
}

impl Duplication {
    fn of_parts(parts: impl IntoIterator<Item = Self>) -> Self {
        if parts.into_iter().any(|d| d == Self::Linear) { Self::Linear } else { Self::Copy }
    }
}

/// The duplication of a type; copy for a type whose parts are not known, so that checks built on
/// it stay silent.
pub struct Kinds;

impl Query for Kinds {
    const NAME: &'static str = "kinds";
    type Key = Ty;
    type Value = Duplication;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let of = |ty: &Ty| cx.get::<Self>(ty);
        match &**ty {
            TyKind::Primitive(Primitive::Mem(Mem::Array | Mem::Slice | Mem::Cell), args) => {
                Duplication::of_parts(args.iter().filter_map(Value::as_type).map(|t| of(&t)))
            }
            TyKind::Primitive(..) => Duplication::Copy,
            TyKind::Tuple(items) => Duplication::of_parts(items.iter().map(of)),
            TyKind::Closure(Closure { captures, .. }) => {
                Duplication::of_parts(captures.iter().map(|(_, t)| of(t)))
            }
            TyKind::Frame(_) => Duplication::Linear,
            TyKind::Nominal(nominal) => Self::nominal(cx, *ty, nominal),
        }
    }

    /// A type whose kind depends on itself, such as a recursive enum, is as its other parts make it.
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        Duplication::Copy
    }
}

impl Kinds {
    fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Duplication {
        let marked = Env::defining(cx, &nominal.literal, nominal.instance.as_ref()).is_some_and(|env| {
            let attrs = env.attrs_of(nominal.literal.expr.id);
            env.attribute(&attrs, Primitive::Kind(Kind::linear)).is_some()
        });
        if marked {
            return Duplication::Linear;
        }
        let Some(members) = cx.get::<Members>(&ty) else { return Duplication::Copy };
        let fields = members.fields.iter().filter_map(|f| f.ty.as_ref().ok());
        let payloads = members.variants.iter().flat_map(|v| v.payload.iter().filter_map(|p| p.as_ref().ok()));
        Duplication::of_parts(fields.chain(payloads).map(|t| cx.get::<Self>(t)))
    }
}
