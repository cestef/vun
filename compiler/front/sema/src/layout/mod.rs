//! Layout of types under the target (spec 05, section 6): sizes and alignments in bytes, the
//! offset of every part, and the unused bit patterns an enum can keep its tag in. Structs keep
//! declaration order with natural alignment and C-style padding; an enum with one payload-free
//! variant and one payload variant whose payload has unused patterns keeps its tag in one of them
//! (the niche rule).

mod exists;
mod frame;
mod nominal;
mod shape;
mod text;

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Cycle, Query};

pub use exists::Exists;
pub use frame::{Framer, Framers};
pub use shape::{Layout, Niche, Parts, Shape, Tag};
pub use text::Text;

use crate::Primitive;
use crate::float::Format;
use crate::int::{Integer, Interval};
use crate::primitives::lang::Lang;
use crate::primitives::mem::Mem;
use crate::target::TargetOf;
use crate::ty::{Closure, Ty, TyKind, Value};

/// How bounds at run time end, the byte after their two bounds: the second bound is the end past
/// the last item, or the last item itself, so no bound needs a value past its items' storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum BoundsEnd {
    /// `a..`: no end.
    Open,
    /// `a..b`: the end past the last item.
    Before,
    /// `a..=b`: the last item.
    Through,
}

impl BoundsEnd {
    /// The byte stored for it.
    #[must_use]
    pub const fn tag(self) -> u128 {
        self as u128
    }
}

/// The shape of a type; `None` for a type without one at run time (`type`, an unbounded
/// integer, a slice, a frame no framer shapes), or with a part without one.
pub struct Shapes;

impl Query for Shapes {
    const NAME: &'static str = "shapes";
    type Key = Ty;
    type Value = Option<Arc<Shape>>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let layouts =
            |tys: &mut dyn Iterator<Item = &Ty>| tys.map(|t| Self::layout(cx, t)).collect::<Option<Vec<_>>>();
        if let Some(range) = ty.interval() {
            return Self::integer(&range).map(|l| Arc::new(Shape::whole(l)));
        }
        let shape = match &**ty {
            TyKind::Primitive(p, args) => Self::primitive(cx, *p, args)?,
            TyKind::Tuple(items) => Shape::sequence(layouts(&mut items.iter())?),
            TyKind::Closure(Closure { captures, .. }) => {
                Shape::sequence(layouts(&mut captures.iter().map(|(_, t)| t))?)
            }
            TyKind::Record(fields) => Shape::sequence(layouts(&mut fields.iter().map(|(_, t)| t))?),
            TyKind::Nominal(nominal) => Self::nominal(cx, *ty, nominal)?,
            TyKind::Frame(_) => Shape::whole(Framers::layout(cx, *ty)?),
            TyKind::Param(_) => return None,
        };
        Some(Arc::new(shape))
    }

    /// Reflecting on a type while defining it (spec 03, section 6).
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        None
    }
}

impl Shapes {
    /// The layout of `ty`, when it has one at run time.
    #[must_use]
    pub fn layout(cx: &Cx<'_>, ty: &Ty) -> Option<Layout> {
        cx.get::<Self>(ty).map(|s| s.layout)
    }

    /// The integers of `range`, in their storage; the patterns past the range are unused.
    fn integer(range: &Interval) -> Option<Layout> {
        let bytes = range.storage()?.bits() / u64::from(u8::BITS);
        let last = range.hi().finite()?;
        Some(Layout::scalar(bytes, Niche::past(bytes, last, &range.count()?)))
    }

    /// How a bound of `Bounds(item)` is stored: as an item, or as a word for items of an
    /// unbounded range, whose bounds at run time are those of what they index or count.
    #[must_use]
    pub fn bound(cx: &Cx<'_>, item: &Ty) -> Option<Layout> {
        match item.interval() {
            Some(range) if range.storage().is_none() => {
                let pointer = Self::pointer(cx)?;
                Some(Layout { niche: None, ..pointer })
            }
            _ => Some(Layout { niche: None, ..Self::layout(cx, item)? }),
        }
    }

    /// A pointer of the target: as wide as an address; never null.
    fn pointer(cx: &Cx<'_>) -> Option<Layout> {
        let usize_max = cx.get::<TargetOf>(&())?.usize_max();
        let bytes = Interval::closed(Integer::zero(), usize_max)?.storage()?.bits() / u64::from(u8::BITS);
        Some(Layout::scalar(bytes, Some(Niche { offset: 0, bytes, start: 0, count: 1 })))
    }

    fn primitive(cx: &Cx<'_>, primitive: Primitive, args: &[Value]) -> Option<Shape> {
        let arg = |i: usize| args.get(i).and_then(Value::as_type);
        match primitive {
            Primitive::Lang(Lang::Float) => {
                let format = Format::of(&TyKind::Primitive(primitive, args.to_vec()))?;
                Some(Shape::whole(Layout::scalar(u64::from(format.bits() / u8::BITS), None)))
            }
            Primitive::Lang(Lang::Fn) | Primitive::Mem(Mem::Ptr) => Self::pointer(cx).map(Shape::whole),
            // A borrow of a slice also holds its length.
            borrow if borrow.borrow_mode().is_some() => {
                let pointer = Self::pointer(cx)?;
                match arg(0)?.slice_items() {
                    Some(_) => Some(Shape::sequence([pointer, Layout { niche: None, ..pointer }])),
                    None => Some(Shape::whole(pointer)),
                }
            }
            Primitive::Mem(Mem::Array) => {
                let (Some(Value::Int(len)), Some(item)) = (args.first(), arg(1)) else { return None };
                let item = Self::layout(cx, &item)?;
                let len = u64::try_from(len.to_i128()?).ok()?;
                let niche = if len == 0 { None } else { item.niche };
                let layout = Layout { size: item.size.checked_mul(len)?, align: item.align, niche };
                Some(Shape { layout, parts: Parts::Array { stride: item.size, len } })
            }
            // Bounds at run time: the low bound, the second bound and how they end
            // (`BoundsEnd`), the bounds in the items' storage, or the word's when they have none.
            Primitive::Lang(Lang::Bounds) => {
                let item = Self::bound(cx, &arg(0)?)?;
                let flag = Layout::scalar(1, None);
                Some(Shape::sequence([item, item, flag]))
            }
            Primitive::Mem(Mem::Cell) => {
                let inner = cx.get::<Self>(&arg(0)?)?;
                Some(Shape {
                    layout: Layout { niche: None, ..inner.layout },
                    parts: Parts::Sequence(vec![0]),
                })
            }
            _ => None,
        }
    }
}
