//! Whether values of a type exist at run time: `type`, code, names, unbounded integers and
//! anything holding one, even through a borrow, exist only at compile time.

use vun_primitives::Primitive;
use vun_primitives::mem::Mem;
use vun_primitives::simd::Simd;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::{Closure, Ty, TyKind, Value};

use super::Shapes;
use crate::member::Members;

pub struct Exists;

impl Query for Exists {
    const NAME: &'static str = "exists";
    type Key = Ty;
    type Value = bool;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        if let Some(pointee) = ty.pointee() {
            return cx.get::<Self>(&pointee);
        }
        match &**ty {
            TyKind::Primitive(
                Primitive::Mem(Mem::Array | Mem::Slice | Mem::Cell) | Primitive::Simd(Simd::Vector),
                args,
            ) => args.iter().filter_map(Value::as_type).all(|t| cx.get::<Self>(&t)),
            TyKind::Primitive(..) | TyKind::Param(_) => cx.get::<Shapes>(ty).is_some(),
            TyKind::Tuple(items) => Self::all(cx, items),
            TyKind::Record(fields) => Self::all(cx, fields.iter().map(|(_, t)| t)),
            TyKind::Closure(Closure { captures, .. }) => Self::all(cx, captures.iter().map(|(_, t)| t)),
            TyKind::Frame(_) => true,
            TyKind::Nominal(_) => cx.get::<Members>(ty).is_none_or(|members| {
                let fields = members.fields.iter().filter_map(|f| f.ty.as_ref().ok());
                let payloads =
                    members.variants.iter().flat_map(|v| v.payload.iter().filter_map(|p| p.as_ref().ok()));
                Self::all(cx, fields.chain(payloads))
            }),
        }
    }

    /// A type holding itself, through a pointer, exists as its other parts make it.
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        true
    }
}

impl Exists {
    fn all<'t>(cx: &Cx<'_>, parts: impl IntoIterator<Item = &'t Ty>) -> bool {
        parts.into_iter().all(|t| cx.get::<Self>(t))
    }
}
