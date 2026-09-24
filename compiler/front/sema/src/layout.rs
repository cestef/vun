//! Layout of types under the target (spec 05, section 6): sizes and alignments in bytes, and the
//! unused bit patterns an enum can keep its tag in. Structs keep declaration order with natural
//! alignment and C-style padding; an enum with one payload-free variant and one payload variant
//! whose payload has unused patterns keeps its tag in one of them (the niche rule).

use overt_query::{Cx, Cycle, Query};

use crate::Primitive;
use crate::env::Env;
use crate::int::{Integer, Interval, Storage};
use crate::member::Members;
use crate::primitives::lang::Lang;
use crate::primitives::layout::Layout as Attr;
use crate::primitives::mem::Mem;
use crate::target::Target;
use crate::ty::{Closure, Nominal, NominalKind, Ty, TyKind, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
    /// How many bit patterns of the size no value uses, saturating.
    pub niches: u128,
}

impl Layout {
    const EMPTY: Self = Self { size: 0, align: 1, niches: 0 };

    /// A scalar of `bytes` bytes, aligned to its size.
    const fn scalar(bytes: u64, niches: u128) -> Self {
        Self { size: bytes, align: bytes, niches }
    }

    /// The integers of `range`, in their storage; the patterns past the range are unused.
    fn integer(range: &Interval) -> Option<Self> {
        let storage = range.storage()?;
        let bits = u32::try_from(storage.bits()).ok()?;
        let unused = &Integer::power_of_two(bits) - &range.count()?;
        Some(Self::scalar(storage.bits() / u64::from(u8::BITS), unused.to_u128().unwrap_or(u128::MAX)))
    }

    /// Parts laid out one after the other, each at its alignment, the whole padded to the
    /// largest: a struct or a tuple. Unused patterns are those of the parts.
    fn sequence(parts: impl IntoIterator<Item = Self>) -> Self {
        let mut whole = Self::EMPTY;
        for part in parts {
            whole.size = whole.size.next_multiple_of(part.align) + part.size;
            whole.align = whole.align.max(part.align);
            whole.niches = whole.niches.max(part.niches);
        }
        whole.size = whole.size.next_multiple_of(whole.align);
        whole
    }
}

/// The layout of a type; `None` for a type without one at run time (`type`, an unbounded
/// integer, a slice, a frame, whose layout is not computed yet), or with a part without one.
pub struct Layouts;

impl Query for Layouts {
    const NAME: &'static str = "layouts";
    type Key = Ty;
    type Value = Option<Layout>;

    fn compute(cx: &Cx<'_>, ty: &Ty) -> Self::Value {
        let layout = |ty: &Ty| cx.get::<Self>(ty);
        if let Some(range) = ty.interval() {
            return Layout::integer(&range);
        }
        match &**ty {
            TyKind::Primitive(p, args) => Self::primitive(cx, *p, args),
            TyKind::Tuple(items) => {
                Some(Layout::sequence(items.iter().map(layout).collect::<Option<Vec<_>>>()?))
            }
            TyKind::Closure(Closure { captures, .. }) => {
                Some(Layout::sequence(captures.iter().map(|(_, t)| layout(t)).collect::<Option<Vec<_>>>()?))
            }
            TyKind::Nominal(nominal) => Self::nominal(cx, *ty, nominal),
            TyKind::Frame(_) => None,
        }
    }

    /// Reflecting on a type while defining it (spec 03, section 6).
    fn cycle(_: &Cx<'_>, _: &Ty, _: &Cycle) -> Self::Value {
        None
    }
}

impl Layouts {
    /// A pointer of the target: as wide as an address; never null.
    fn pointer(cx: &Cx<'_>) -> Option<Layout> {
        let usize_max = cx.db().resource::<Target>().usize_max.clone();
        let words = Interval::closed(Integer::zero(), usize_max)?.storage()?;
        Some(Layout::scalar(words.bits() / u64::from(u8::BITS), 1))
    }

    fn primitive(cx: &Cx<'_>, primitive: Primitive, args: &[Value]) -> Option<Layout> {
        let arg = |i: usize| args.get(i).and_then(Value::as_type);
        let float = |storage: Storage| Some(Layout::scalar(storage.bits() / u64::from(u8::BITS), 0));
        match primitive {
            Primitive::Lang(Lang::f16) => float(Storage::B16),
            Primitive::Lang(Lang::f32) => float(Storage::B32),
            Primitive::Lang(Lang::f64) => float(Storage::B64),
            Primitive::Lang(Lang::Fn) | Primitive::Mem(Mem::Ptr) => Self::pointer(cx),
            // A borrow of a slice also holds its length.
            Primitive::Mem(Mem::Ref | Mem::RefMut | Mem::RefOut | Mem::RefTake) => {
                let pointer = Self::pointer(cx)?;
                match arg(0)?.slice_items() {
                    Some(_) => Some(Layout::sequence([pointer, Layout { niches: 0, ..pointer }])),
                    None => Some(pointer),
                }
            }
            Primitive::Mem(Mem::Array) => {
                let (Some(Value::Int(len)), Some(item)) = (args.first(), arg(1)) else { return None };
                let item = cx.get::<Self>(&item)?;
                let len = u64::try_from(len.to_i128()?).ok()?;
                let niches = if len == 0 { 0 } else { item.niches };
                Some(Layout { size: item.size.checked_mul(len)?, align: item.align, niches })
            }
            Primitive::Mem(Mem::Cell) => Some(Layout { niches: 0, ..cx.get::<Self>(&arg(0)?)? }),
            _ => None,
        }
    }

    fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Option<Layout> {
        let members = cx.get::<Members>(&ty)?;
        if !members.complete {
            return None;
        }
        let env = Env::defining(cx, &nominal.literal, nominal.instance.as_ref())?;
        let attrs = env.attrs_of(nominal.literal.expr.id);
        let attr = |a: Attr| env.attribute(&attrs, Primitive::Layout(a));
        let layout = |ty: &Ty| cx.get::<Self>(ty);
        let whole = match nominal.kind {
            NominalKind::Empty => Layout::EMPTY,
            // `#[layout.bits(uN)]`: exactly the backing integer.
            NominalKind::Struct if let Some([backing]) = attr(Attr::bits).as_deref() => {
                Layout { niches: 0, ..layout(&backing.as_type()?)? }
            }
            NominalKind::Struct => {
                let fields =
                    members.fields.iter().map(|f| layout(f.ty.as_ref().ok()?)).collect::<Option<Vec<_>>>()?;
                Layout::sequence(fields)
            }
            NominalKind::Enum => {
                let payloads = members.variants.iter().map(|v| {
                    Some(Layout::sequence(
                        v.payload.iter().map(|p| layout(p.as_ref().ok()?)).collect::<Option<Vec<_>>>()?,
                    ))
                });
                let tag = attr(Attr::tag).and_then(|t| layout(&t.first()?.as_type()?));
                Self::tagged(
                    &payloads.collect::<Option<Vec<_>>>()?,
                    tag,
                    &members.variants.iter().map(|v| v.payload.is_empty()).collect::<Vec<_>>(),
                )?
            }
        };
        // `#[layout.align(n)]` raises the alignment.
        match attr(Attr::align).as_deref() {
            Some([Value::Int(n)]) => {
                let align = u64::try_from(n.to_i128()?).ok()?.max(whole.align);
                Some(Layout { size: whole.size.next_multiple_of(align), align, ..whole })
            }
            _ => Some(whole),
        }
    }

    /// An enum of variants with the payloads `payloads` (`empty` tells which carry none), its tag
    /// in `tag` when an attribute sets it: in a niche of the one payload when the niche rule
    /// applies, else before the largest payload.
    fn tagged(payloads: &[Layout], tag: Option<Layout>, empty: &[bool]) -> Option<Layout> {
        let count = payloads.len();
        if let ([true, false] | [false, true], None) = (empty, tag) {
            let payload = payloads[usize::from(empty[0])];
            if payload.niches > 0 {
                return Some(Layout { niches: payload.niches - 1, ..payload });
            }
        }
        let tag = match tag {
            Some(tag) => tag,
            None if count <= 1 => Layout::EMPTY,
            None => Layout::integer(&Interval::closed(Integer::zero(), Integer::from(count - 1))?)?,
        };
        let widest = payloads.iter().map(|p| p.size).max().unwrap_or(0);
        let align = payloads.iter().map(|p| p.align).fold(tag.align, u64::max);
        let size = (tag.size.next_multiple_of(align) + widest).next_multiple_of(align);
        // The tag's patterns past the variants are unused.
        let patterns = Integer::power_of_two(u32::try_from(tag.size * u64::from(u8::BITS)).ok()?);
        let unused = if tag.size == 0 { Integer::zero() } else { &patterns - &Integer::from(count) };
        Some(Layout { size, align, niches: unused.to_u128().unwrap_or(u128::MAX) })
    }
}

impl Layouts {
    /// How many bits a field of type `ty` takes in a `#[layout.bits(uN)]` struct: the minimal
    /// count of an integer's range (spec 04, rule 7), of an enum's tags, of the backing of a
    /// nested bit struct; whole bytes otherwise.
    #[must_use]
    pub fn bits(cx: &Cx<'_>, ty: Ty) -> Option<u64> {
        if let Some(range) = ty.interval() {
            return range.bits();
        }
        if let TyKind::Nominal(nominal) = &*ty {
            let members = cx.get::<Members>(&ty)?;
            let env = Env::defining(cx, &nominal.literal, nominal.instance.as_ref())?;
            let attrs = env.attrs_of(nominal.literal.expr.id);
            if let Some([backing]) = env.attribute(&attrs, Primitive::Layout(Attr::bits)).as_deref() {
                return Self::bits(cx, backing.as_type()?);
            }
            if nominal.kind == NominalKind::Enum && members.variants.iter().all(|v| v.payload.is_empty()) {
                let last = Integer::from(members.variants.len().checked_sub(1)?);
                return Interval::closed(Integer::zero(), last)?.bits();
            }
        }
        Some(cx.get::<Self>(&ty)?.size * u64::from(u8::BITS))
    }
}
