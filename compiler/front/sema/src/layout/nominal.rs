//! Layouts of `struct` and `enum` literals, with their layout attributes: `#[layout.bits(uN)]`,
//! `#[layout.tag(uN)]` and `#[layout.align(N)]`.

use vun_query::Cx;

use super::{Layout, Niche, Parts, Shape, Shapes, Tag};
use crate::Primitive;
use crate::env::Env;
use crate::int::{Integer, Interval};
use crate::member::Members;
use crate::primitives::layout::Layout as Attr;
use crate::ty::{Nominal, NominalKind, Ty, Value};

impl Shapes {
    pub(super) fn nominal(cx: &Cx<'_>, ty: Ty, nominal: &Nominal) -> Option<Shape> {
        let members = cx.get::<Members>(&ty)?;
        if !members.complete {
            return None;
        }
        let env = Env::defining(cx, &nominal.literal, nominal.instance.as_ref())?;
        let attrs = env.attrs_of(nominal.literal.expr.id);
        let attr = |a: Attr| env.attribute(&attrs, Primitive::Layout(a));
        let layout = |ty: &Ty| Self::layout(cx, ty);
        let whole = match nominal.kind {
            NominalKind::Empty => Shape::whole(Layout::EMPTY),
            // `#[layout.bits(uN)]`: exactly the backing integer, the fields from its least
            // significant bit on.
            NominalKind::Struct if let Some([backing]) = attr(Attr::bits).as_deref() => {
                let mut offsets = Vec::new();
                let mut at = 0;
                for field in &members.fields {
                    offsets.push(at);
                    at += Self::bits(cx, *field.ty.as_ref().ok()?)?;
                }
                let layout = Layout { niche: None, ..layout(&backing.as_type()?)? };
                Shape { layout, parts: Parts::Bits(offsets) }
            }
            NominalKind::Struct => Shape::sequence(
                members.fields.iter().map(|f| layout(f.ty.as_ref().ok()?)).collect::<Option<Vec<_>>>()?,
            ),
            NominalKind::Enum => {
                let payloads = members.variants.iter().map(|v| {
                    Some(Layout::sequence(
                        v.payload.iter().map(|p| layout(p.as_ref().ok()?)).collect::<Option<Vec<_>>>()?,
                    ))
                });
                let tag = attr(Attr::tag).and_then(|t| layout(&t.first()?.as_type()?));
                let empty: Vec<bool> = members.variants.iter().map(|v| v.payload.is_empty()).collect();
                Self::tagged(&payloads.collect::<Option<Vec<_>>>()?, tag, &empty)?
            }
        };
        // `#[layout.align(n)]` raises the alignment.
        match attr(Attr::align).as_deref() {
            Some([Value::Int(n)]) => {
                let align = u64::try_from(n.to_i128()?).ok()?.max(whole.layout.align);
                let layout =
                    Layout { size: whole.layout.size.next_multiple_of(align), align, ..whole.layout };
                Some(Shape { layout, ..whole })
            }
            _ => Some(whole),
        }
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
        let base = tag.size.next_multiple_of(align);
        let size = (base + widest).next_multiple_of(align);
        let placed = payloads.iter().map(|(_, offsets)| offsets.iter().map(|o| base + o).collect()).collect();
        // The tag's patterns past the variants are unused.
        let (tag, niche) = if tag.size == 0 {
            (Tag::Alone, None)
        } else {
            let last = &Integer::from(count) - &Integer::from(1);
            (Tag::Index { bytes: tag.size }, Niche::past(tag.size, &last, &Integer::from(count)))
        };
        Some(Shape { layout: Layout { size, align, niche }, parts: Parts::Enum { tag, payloads: placed } })
    }

    /// How many bits a field of type `ty` takes in a `#[layout.bits(uN)]` struct: the minimal
    /// count of an integer's range (spec 04, rule 7), of an enum's tags, of the backing of a
    /// nested bit struct; whole bytes otherwise.
    #[must_use]
    pub fn bits(cx: &Cx<'_>, ty: Ty) -> Option<u64> {
        if let Some(range) = ty.interval() {
            return range.bits();
        }
        if let crate::ty::TyKind::Nominal(nominal) = &*ty {
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
        Some(Self::layout(cx, &ty)?.size * u64::from(u8::BITS))
    }
}
