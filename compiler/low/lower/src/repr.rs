//! How values of each type are held in LIR: nothing for a type of size 0, a scalar for integers,
//! floats, addresses and what is stored as one of them, memory for everything else.

use std::sync::Arc;

use vun_lir::Scalar;
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::Primitive;
use vun_sema::layout::{Layout, Parts, Shape, Shapes, Tag};
use vun_sema::member::Members;
use vun_sema::primitives::lang::Lang;
use vun_sema::primitives::mem::Mem;
use vun_sema::target::Target;
use vun_sema::ty::{Ty, TyKind};

use crate::unlowered::{Unlowered, Why};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repr {
    /// Nothing is stored.
    Zero,
    Scalar(Scalar),
    Memory(Layout),
}

/// The representations of types on the target of a compilation.
#[derive(Clone, Copy)]
pub struct Types<'a, 'db> {
    pub cx: &'a Cx<'db>,
}

impl Types<'_, '_> {
    #[must_use]
    pub fn pointer_bits(self) -> u32 {
        self.cx.db().resource::<Target>().address_bits()
    }

    /// The integer as wide as an address.
    #[must_use]
    pub fn word(self) -> Scalar {
        Scalar::int(self.pointer_bits()).unwrap_or(Scalar::I64)
    }

    /// The shape of `ty`, needed at `at`.
    ///
    /// # Errors
    /// Returns [`Why::NoLayout`] for a type with no layout at run time.
    pub fn shape(self, ty: Ty, at: OriginId) -> Result<Arc<Shape>, Unlowered> {
        self.cx.get::<Shapes>(&ty).ok_or(Unlowered { at, why: Why::NoLayout(ty) })
    }

    /// The integer storing values of the range type `ty`, and whether it is signed.
    #[must_use]
    pub fn int(self, ty: Ty) -> Option<(Scalar, bool)> {
        let range = ty.interval()?;
        let bits = u32::try_from(range.storage()?.bits()).ok()?;
        Some((Scalar::int(bits)?, range.is_signed()))
    }

    /// How values of type `ty` are held.
    ///
    /// # Errors
    /// Returns [`Why::NoLayout`] for a type with no layout at run time.
    pub fn repr(self, ty: Ty, at: OriginId) -> Result<Repr, Unlowered> {
        if let Some((int, _)) = self.int(ty) {
            return Ok(Repr::Scalar(int));
        }
        let float = match &*ty {
            TyKind::Primitive(Primitive::Lang(Lang::f16), _) => Some(Scalar::F16),
            TyKind::Primitive(Primitive::Lang(Lang::f32), _) => Some(Scalar::F32),
            TyKind::Primitive(Primitive::Lang(Lang::f64), _) => Some(Scalar::F64),
            _ => None,
        };
        if let Some(float) = float {
            return Ok(Repr::Scalar(float));
        }
        let shape = self.shape(ty, at)?;
        if shape.layout.size == 0 {
            return Ok(Repr::Zero);
        }
        let address = match &*ty {
            TyKind::Primitive(Primitive::Lang(Lang::Fn) | Primitive::Mem(Mem::Ptr), _) => true,
            TyKind::Primitive(p, _) if p.borrow_mode().is_some() => shape.parts == Parts::Whole,
            _ => false,
        };
        if address {
            return Ok(Repr::Scalar(Scalar::Ptr));
        }
        if let Parts::Enum { tag, payloads } = &shape.parts {
            match *tag {
                // Variants alone: the tag.
                Tag::Index { bytes } if payloads.iter().all(Vec::is_empty) && shape.layout.size == bytes => {
                    let bits = u32::try_from(bytes * u64::from(u8::BITS)).ok();
                    if let Some(int) = bits.and_then(Scalar::int) {
                        return Ok(Repr::Scalar(int));
                    }
                }
                // A payload stored as a scalar, its unused patterns telling the other variant:
                // `?*T` is an address.
                Tag::Niche { empty, offset: 0, bytes, .. } if shape.layout.size == bytes => {
                    if let Some(payload) = self.payload(ty, 1 - empty)
                        && let Repr::Scalar(scalar) = self.repr(payload, at)?
                    {
                        return Ok(Repr::Scalar(scalar));
                    }
                }
                _ => {}
            }
        }
        Ok(Repr::Memory(shape.layout))
    }

    /// The type of the single item of the payload of the variant at `index` of the enum `ty`.
    fn payload(self, ty: Ty, index: usize) -> Option<Ty> {
        let members = self.cx.get::<Members>(&ty)?;
        match members.variants.get(index)?.payload.as_slice() {
            [Ok(item)] => Some(*item),
            _ => None,
        }
    }
}
