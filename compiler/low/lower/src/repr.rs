//! How values of each type are held in LIR: nothing for a type of size 0, a scalar for integers,
//! floats, addresses and what is stored as one of them, memory for everything else.

use std::sync::Arc;

use vun_lir::Scalar;
use vun_origin::OriginId;
use vun_query::Cx;
use vun_sema::Primitive;
use vun_sema::int::Interval;
use vun_sema::layout::{Layout, Parts, Shape, Shapes, Tag};
use vun_sema::member::Members;
use vun_sema::primitives::lang::Lang;
use vun_sema::primitives::mem::Mem;
use vun_sema::target::Target;
use vun_sema::ty::{Ty, TyKind};
use vun_target::Endian;

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
    /// The order of the bytes of a value in memory.
    pub endian: Endian,
    /// The integer as wide as an address.
    pub word: Scalar,
}

impl<'a, 'db> Types<'a, 'db> {
    /// The representations on `target`, whose addresses must be as wide as an integer LIR has.
    ///
    /// # Errors
    /// Returns the width of addresses when no integer of LIR is that wide.
    pub fn new(cx: &'a Cx<'db>, target: &Target) -> Result<Self, u32> {
        let word = Scalar::int(target.address_bits).ok_or(target.address_bits)?;
        Ok(Self { cx, endian: target.endian, word })
    }

    #[must_use]
    pub fn pointer_bits(self) -> u32 {
        self.word.bits(0)
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
        Self::storing(&ty.interval()?)
    }

    /// The integer storing both the values of `a` and those of `b`, and whether it is signed:
    /// where two integers are compared.
    #[must_use]
    pub fn common(self, a: Ty, b: Ty) -> Option<(Scalar, bool)> {
        Self::storing(&a.interval()?.hull(&b.interval()?))
    }

    fn storing(range: &Interval) -> Option<(Scalar, bool)> {
        let bits = u32::try_from(range.storage()?.bits()).ok()?;
        Some((Scalar::int(bits)?, range.is_signed()))
    }

    /// How values of type `ty` are held.
    ///
    /// # Errors
    /// Returns [`Why::NoLayout`] for a type with no layout at run time.
    pub fn repr(self, ty: Ty, at: OriginId) -> Result<Repr, Unlowered> {
        if let TyKind::Frame(_) = &*ty {
            return match &*self.cx.get::<crate::frame::Frames>(&ty) {
                Ok(shape) => Ok(Repr::Memory(shape.layout)),
                Err(unlowered) => Err(unlowered.clone()),
            };
        }
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
