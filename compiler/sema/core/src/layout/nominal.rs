//! Layouts of `struct` and `enum` literals, with their layout attributes: `#[layout.bits(uN)]`,
//! `#[layout.tag(uN)]`, `#[layout.align(N)]` and a field's `#[layout.offset(N)]`.

use vun_primitives::Primitive;
use vun_primitives::layout::Layout as Attr;
use vun_query::Cx;
use vun_sema_int::{Integer, Interval};
use vun_sema_ty::{Nominal, NominalKind, Ty, Value};

use super::{Layout, Niche, Parts, Shape, Shapes, Tag};
use crate::attributes::Attributed;
use crate::member::{MemberTable, Members};

impl Shapes {
    pub(super) fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Option<Shape> {
        let members = cx.get::<Members>(&ty)?;
        if !members.complete {
            return None;
        }
        let attributes = nominal.attributes(cx)?;
        let attr = |a: Attr| attributes.get(Primitive::Layout(a));
        let layout = |ty: &Ty| Self::layout(cx, ty);
        let whole = match nominal.kind {
            NominalKind::Empty => Shape::whole(Layout::EMPTY),
            // `#[layout.bits(uN)]`: exactly the backing integer, the fields from its least
            // significant bit on.
            NominalKind::Struct if let Some([backing]) = attr(Attr::bits).as_deref() => {
                let mut offsets = Vec::new();
                let mut at = 0u64;
                for field in &members.fields {
                    offsets.push(at);
                    at = at.checked_add(Self::bits(cx, *field.ty.as_ref().ok()?)?)?;
                }
                // Fields past the backing integer have no place in it.
                let backing = backing.as_type()?;
                if at > Self::bits(cx, backing)? {
                    return None;
                }
                let layout = Layout { niche: None, ..layout(&backing)? };
                Shape { layout, parts: Parts::Bits(offsets) }
            }
            // `#[layout.offset(n)]` places a field at `n`.
            NominalKind::Struct => {
                let parts = members.fields.iter().map(|f| Some((layout(f.ty.as_ref().ok()?)?, f.offset)));
                let (layout, offsets) = Layout::placed(parts.collect::<Option<Vec<_>>>()?)?;
                Shape { layout, parts: Parts::Sequence(offsets) }
            }
            NominalKind::Enum if let Ok(Some(codes)) = members.codes() => {
                Self::coded(cx, &members, &codes, attr(Attr::tag).as_deref())?
            }
            NominalKind::Enum if members.codes().is_err() => return None,
            NominalKind::Enum => {
                let payloads = members.variants.iter().map(|v| {
                    Some(Layout::sequence(
                        v.payload.iter().map(|p| layout(p.as_ref().ok()?)).collect::<Option<Vec<_>>>()?,
                    ))
                });
                let tag = match attr(Attr::tag).as_deref() {
                    Some([tag]) => Some(Self::tag(cx, tag.as_type()?, members.variants.len())?),
                    _ => None,
                };
                let empty: Vec<bool> = members.variants.iter().map(|v| v.payload.is_empty()).collect();
                Self::tagged(&payloads.collect::<Option<Vec<_>>>()?, tag, &empty)?
            }
        };
        // `#[layout.align(n)]` raises the alignment to a power of two.
        match attr(Attr::align).as_deref() {
            Some([Value::Int(n)]) => {
                let align =
                    n.to_u128().and_then(|n| u64::try_from(n).ok()).filter(|n| n.is_power_of_two())?;
                let align = align.max(whole.layout.align);
                let size = whole.layout.size.checked_next_multiple_of(align)?;
                let layout = Layout { size, align, ..whole.layout };
                Some(Shape { layout, ..whole })
            }
            _ => Some(whole),
        }
    }

    /// An enum whose variants name their tags (`name = code`), which `codes` lists: no variant
    /// holds anything, and each tag is an integer of `#[layout.tag(T)]`, or of the least type
    /// holding every code.
    fn coded(cx: &Cx<'_>, members: &MemberTable, codes: &[Integer], tag: Option<&[Value]>) -> Option<Shape> {
        if members.variants.iter().any(|v| !v.payload.is_empty()) {
            return None;
        }
        let (low, high) = codes.iter().fold((codes.first()?, codes.first()?), |(low, high), c| {
            (if c < low { c } else { low }, if c > high { c } else { high })
        });
        let range = Interval::closed(low.clone(), high.clone())?;
        let tag = match tag {
            Some([tag]) => {
                let tag = tag.as_type()?;
                if !tag.interval()?.contains(&range) {
                    return None;
                }
                Self::layout(cx, &tag)?
            }
            _ => Shapes::integer(&range)?,
        };
        let patterns = codes.iter().map(|c| Self::pattern(c, tag.size)).collect::<Option<Vec<_>>>()?;
        let payloads = vec![Vec::new(); codes.len()];
        Some(Shape {
            layout: Layout { size: tag.size, align: tag.align, niche: None },
            parts: Parts::Enum { tag: Tag::Index { bytes: tag.size, codes: Some(patterns) }, payloads },
        })
    }

    /// The bit pattern `code` is in an integer of `bytes` bytes, two's complement when negative.
    fn pattern(code: &Integer, bytes: u64) -> Option<u128> {
        let value = code.to_i128().map(i128::cast_unsigned).or_else(|| code.to_u128())?;
        let bits = bytes.checked_mul(8)?;
        Some(if bits >= 128 { value } else { value & ((1u128 << bits) - 1) })
    }

    /// An enum of variants with the payloads `payloads`, each placed from offset 0 (`empty`
    /// tells which carry none), its tag in `tag` when an attribute sets it: in a niche of the one
    /// payload when the niche rule applies, else before the largest payload.
    fn tagged(payloads: &[(Layout, Vec<u64>)], tag: Option<Layout>, empty: &[bool]) -> Option<Shape> {
        let count = payloads.len();
        if let ([true, false] | [false, true], None) = (empty, tag) {
            let full = usize::from(empty[0]);
            let (payload, offsets) = &payloads[full];
            if let Some(niche) = payload.niche {
                let (value, rest) = niche.take();
                let tag = Tag::Niche { empty: 1 - full, offset: niche.offset, bytes: niche.bytes, value };
                let mut placed = vec![Vec::new(); count];
                placed[full].clone_from(offsets);
                let layout = Layout { niche: rest, ..*payload };
                return Some(Shape { layout, parts: Parts::Enum { tag, payloads: placed } });
            }
        }
        let tag = match tag {
            Some(tag) => tag,
            None if count <= 1 => Layout::EMPTY,
            None => Shapes::integer(&Interval::closed(Integer::zero(), Integer::from(count - 1))?)?,
        };
        let widest = payloads.iter().map(|(p, _)| p.size).max().unwrap_or(0);
        let align = payloads.iter().map(|(p, _)| p.align).fold(tag.align, u64::max);
        let base = tag.size.checked_next_multiple_of(align)?;
        let size = base.checked_add(widest)?.checked_next_multiple_of(align)?;
        // Every offset lies within the widest payload, so none passes `size`.
        let placed = payloads.iter().map(|(_, offsets)| offsets.iter().map(|o| base + o).collect()).collect();
        // The tag's patterns past the variants are unused.
        let (tag, niche) = if tag.size == 0 {
            (Tag::Alone, None)
        } else {
            let last = &Integer::from(count) - &Integer::from(1);
            (Tag::Index { bytes: tag.size, codes: None }, Niche::past(tag.size, &last, &Integer::from(count)))
        };
        Some(Shape { layout: Layout { size, align, niche }, parts: Parts::Enum { tag, payloads: placed } })
    }

    /// The layout of `#[layout.tag(T)]` on an enum of `count` variants: `T` a `uN` type with a
    /// pattern for each; `None` for another.
    fn tag(cx: &Cx<'_>, ty: Ty, count: usize) -> Option<Layout> {
        let range = ty.interval().filter(|r| r.unsigned_width().is_some())?;
        (range.count()? >= Integer::from(count)).then(|| Self::layout(cx, &ty)).flatten()
    }

    /// How many bits a field of type `ty` takes in a `#[layout.bits(uN)]` struct: the minimal
    /// count of an integer's range (spec 04, rule 7), of an enum's tags, of the backing of a
    /// nested bit struct; whole bytes otherwise.
    #[must_use]
    pub fn bits(cx: &Cx<'_>, ty: Ty) -> Option<u64> {
        if let Some(range) = ty.interval() {
            return range.bits();
        }
        if let vun_sema_ty::TyKind::Nominal(nominal) = &*ty {
            let members = cx.get::<Members>(&ty)?;
            if let Some([backing]) = nominal.attributes(cx)?.get(Primitive::Layout(Attr::bits)).as_deref() {
                return Self::bits(cx, backing.as_type()?);
            }
            if nominal.kind == NominalKind::Enum && members.variants.iter().all(|v| v.payload.is_empty()) {
                let last = Integer::from(members.variants.len().checked_sub(1)?);
                return Interval::closed(Integer::zero(), last)?.bits();
            }
        }
        Some(Self::layout(cx, &ty)?.size * u64::from(u8::BITS))
    }
}
