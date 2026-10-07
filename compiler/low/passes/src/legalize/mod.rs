//! Legalizing the scalars a backend does not compute with (spec 13, section 8.1), before it reads
//! the unit.
//!
//! An integer wider than the widest a backend computes with becomes limbs of the widest, low
//! first, and every operation on it a sequence over the limbs. A wide parameter is passed as its
//! limbs, one `AbiType::Wide` the backend's convention places together, and a wide result
//! written through an address as its limbs, which the C convention returns in registers as C
//! returns its wide integers, and Vun's through memory. Division calls a helper function the
//! pass adds, one per count of limbs. Afterwards no value is wider than the widest width.
//!
//! A division or remainder of a narrower integer, for a backend whose processor has no division
//! instruction, calls the helper of one limb, its operands extended to the widest first.
//!
//! An `f16`, for a backend that does not compute it, becomes its bits, an `i16`, and every
//! operation on it one in `f32` between an exact widening and a rounding back (`float16`).
//!
//! A float the backend does not hold, the target's processor not computing it, becomes its bits:
//! every operation on it ran as `lang.float.soft`'s before lowering, so only moves of its bits
//! remain, and an `f64` on a 32-bit target is two limbs like an `i64`. A C function passes one
//! only under a convention passing floats in general registers, as integers of their width.

mod divide;
mod float16;
mod limbs;
mod rewrite;
#[cfg(test)]
mod tests;

use std::fmt;

use vun_abi::{Floats as Passed, Rules};
use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_origin::OriginId;
use vun_target::{Convention, Endian, Floats};

use rewrite::Rewrite;
use vun_lir::{AbiType, Conv, Ext, FuncId, ImportKind, Scalar, Signature, Unit};

/// How a unit is legalized for a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Legalize {
    /// The widest integer the backend computes with.
    pub widest: Scalar,
    /// Where the limbs of a wide integer are in memory.
    pub endian: Endian,
    pub half: Half,
    /// The floats the backend holds as floats.
    pub floats: Floats,
    /// The convention the target's C functions follow.
    pub convention: Convention,
    pub division: Division,
}

/// How a backend divides integers no wider than its widest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Division {
    /// By an instruction: the target's processor's, or the one a compiler it hands its code to
    /// chooses.
    Computed,
    /// Through the helper wide integers divide by, restoring the quotient one bit at a time: the
    /// processor has no division instruction.
    Restoring,
}

/// How a backend holds `f16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Half {
    /// As a float it computes with.
    Computed,
    /// As its bits, each operation computed in `f32`.
    Promoted,
}

/// Why an operation on a wide integer has no sequence over limbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Unlegalized {
    /// An atomic access of a wide integer.
    Atomic,
    /// A conversion between a wide integer and a float.
    Float,
    /// A conversion of a promoted `f16` from or to anything but `f32`, `f64` and its bits.
    HalfConversion,
    /// A promoted `f16` passed under the C convention, which passes it as a float.
    HalfAbi,
    /// A float held as its bits passed under a C convention passing it in a float register.
    FloatAbi,
    /// An operation, other than a move of its bits, on a float held as its bits.
    HeldFloat,
}

/// An operation the pass cannot legalize, where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unlegal {
    pub at: OriginId,
    pub why: Unlegalized,
}

impl fmt::Display for Unlegal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.why {
            Unlegalized::Atomic => "an atomic access of an integer wider than the backend's",
            Unlegalized::Float => "a conversion between a float and an integer wider than the backend's",
            Unlegalized::HalfConversion => "a conversion to `f16` from a type other than `f32`",
            Unlegalized::HalfAbi => "an `f16` passed under the C convention",
            Unlegalized::FloatAbi => {
                "a float its processor does not compute passed to or from C in a float register"
            }
            Unlegalized::HeldFloat => "an operation on a float its processor does not compute",
        })
    }
}

impl std::error::Error for Unlegal {}

/// The helper functions the pass adds to a unit, made once each when first called.
pub(super) struct Helpers {
    /// The id the next helper gets: after every function of the unit.
    next: FuncId,
    /// The unsigned divisions giving quotient and remainder, in the order made: of integers of
    /// how many limbs, and where each was first needed.
    divide: Vec<(u8, OriginId)>,
}

impl Helpers {
    /// The division helper of integers of `limbs` limbs, made when first called.
    pub(super) fn divide(&mut self, limbs: u8, at: OriginId) -> FuncId {
        let made = self.divide.iter().position(|&(l, _)| l == limbs).unwrap_or_else(|| {
            self.divide.push((limbs, at));
            self.divide.len() - 1
        });
        FuncId::new(self.next.index() + made)
    }
}

impl Legalize {
    /// How many limbs a value of type `ty` is, where it is an integer wider than the widest or
    /// the bits of a float held so.
    fn limbs(self, ty: Scalar) -> Option<u8> {
        let ty = self.held(ty).unwrap_or(ty);
        let (bits, widest) = (ty.bits(0), self.widest.bits(0));
        (ty.is_int() && bits > widest).then(|| u8::try_from(bits / widest).expect("a few limbs"))
    }

    /// The integer holding the bits of a value of `ty`, where it is a float the backend does not
    /// hold.
    fn held(self, ty: Scalar) -> Option<Scalar> {
        let bits = ty.bits(0);
        (ty.is_float() && !self.floats.computes(bits)).then(|| Scalar::int(bits).expect("a float's width"))
    }

    /// Whether a value of `ty` is an `f16` held as its bits and computed in `f32`.
    fn promoted(self, ty: Scalar) -> bool {
        ty == Scalar::F16 && self.half == Half::Promoted && self.held(ty).is_none()
    }

    /// The scalar a value of `ty`, not wide, becomes.
    fn scalar(self, ty: Scalar) -> Scalar {
        if self.promoted(ty) { Scalar::I16 } else { self.held(ty).unwrap_or(ty) }
    }

    fn limb_bytes(self) -> u64 {
        self.widest.bytes(0)
    }

    /// The offset in memory of limb `k` of a wide integer of `count` limbs.
    fn offset(self, k: usize, count: usize) -> u64 {
        let place = match self.endian {
            Endian::Little => k,
            Endian::Big => count - 1 - k,
        };
        self.limb_bytes() * u64::try_from(place).expect("a few limbs")
    }

    /// `signature` with each wide integer, parameter or result, passed as its limbs, and each
    /// promoted `f16` as its bits.
    fn signature(self, signature: &Signature) -> Result<Signature, Unlegalized> {
        let half = |t: &AbiType| match t {
            AbiType::Scalar(s) => self.promoted(*s),
            AbiType::Record(r) => r.fields.iter().any(|&(_, s)| self.promoted(s)),
            _ => false,
        };
        let passed = signature.params.iter().chain(&signature.result);
        if signature.conv == Conv::C && passed.clone().any(half) {
            return Err(Unlegalized::HalfAbi);
        }
        let held = |t: &AbiType| match t {
            AbiType::Scalar(s) => self.held(*s).is_some(),
            AbiType::Record(r) => r.fields.iter().any(|&(_, s)| self.held(s).is_some()),
            _ => false,
        };
        let core = Rules::of(self.convention).is_some_and(|r| r.calling(signature).floats == Passed::Core);
        if signature.conv == Conv::C && !core && passed.clone().any(held) {
            return Err(Unlegalized::FloatAbi);
        }
        let promoted = |t: AbiType| match t {
            AbiType::Scalar(s) if self.promoted(s) => AbiType::Int(Scalar::I16, Ext::Zero),
            AbiType::Scalar(s) if let Some(bits) = self.held(s) => AbiType::Int(bits, Ext::Zero),
            AbiType::Record(mut r) => {
                for (_, s) in &mut r.fields {
                    *s = self.scalar(*s);
                }
                AbiType::Record(r)
            }
            other => other,
        };
        let split = |t: &AbiType| match promoted(t.clone()) {
            AbiType::Int(s, ext) => match self.limbs(s) {
                Some(limbs) => AbiType::Wide { limb: self.widest, limbs, ext },
                None => AbiType::Int(s, ext),
            },
            other => other,
        };
        let result = signature.result.as_ref().map(split);
        Ok(Signature {
            conv: signature.conv,
            params: signature.params.iter().map(split).collect(),
            result,
            variadic: signature.variadic,
            convention: signature.convention,
        })
    }

    /// Splits every integer of `unit` wider than `self.widest` into limbs; see the module.
    ///
    /// # Errors
    /// Returns the first operation on a wide integer that has no sequence over limbs.
    pub fn apply(self, unit: &mut Unit) -> Result<(), Unlegal> {
        let legalize = self;
        if legalize.widest == Scalar::I128
            && legalize.half == Half::Computed
            && legalize.floats == Floats::ALL
            && legalize.division == Division::Computed
        {
            return Ok(());
        }
        let mut helpers = Helpers { next: FuncId::new(unit.functions.len()), divide: Vec::new() };
        let rewritten = unit
            .functions
            .iter()
            .map(|(_, f)| Rewrite::function(unit, f, legalize, &mut helpers))
            .collect::<Result<Vec<_>, _>>()?;
        for (id, f) in rewritten.into_iter().enumerate() {
            unit.functions[FuncId::new(id)] = f;
        }
        for (_, import) in unit.imports.iter_mut() {
            if let ImportKind::Func(signature) = &import.kind {
                let signature =
                    legalize.signature(signature).map_err(|why| Unlegal { at: import.origin, why })?;
                import.kind = ImportKind::Func(signature);
            }
        }
        for (limbs, at) in helpers.divide {
            unit.functions.push(divide::helper(legalize, limbs, unit.pointer_bits, at));
        }
        unit.verified();
        Ok(())
    }
}
