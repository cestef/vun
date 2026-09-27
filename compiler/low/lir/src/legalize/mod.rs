//! Legalizing the scalars a backend does not compute with (spec 13, section 8.1), before it reads
//! the unit.
//!
//! An integer of twice the widest width a backend computes with becomes a pair of halves, low
//! first, and every operation on it a sequence over the halves. A wide parameter is passed as its
//! two halves, one `AbiType::Wide` the backend's convention places together; a wide result is a record of them, which the C convention returns in registers as
//! C returns its 128-bit integers, and Vun's through memory. Division calls a helper function the
//! pass adds. Afterwards no value is wider than the widest width.
//!
//! An `f16`, for a backend that does not compute it, becomes its bits, an `i16`, and every
//! operation on it one in `f32` between an exact widening and a rounding back (`float16`).

mod divide;
mod float16;
mod halves;
mod rewrite;
#[cfg(test)]
mod tests;

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_index::Idx;
use vun_origin::OriginId;
use vun_target::Endian;

use crate::func::Record;
use crate::{AbiType, Conv, Ext, FuncId, ImportKind, Scalar, Signature, Unit};
use rewrite::Rewrite;

/// How a unit is legalized for a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Legalize {
    /// The widest integer the backend computes with.
    pub widest: Scalar,
    /// Where the halves of a wide integer are in memory.
    pub endian: Endian,
    pub half: Half,
}

/// How a backend holds `f16`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum Half {
    /// As a float it computes with.
    Computed,
    /// As its bits, each operation computed in `f32`.
    Promoted,
}

/// Why an operation on a wide integer has no sequence over halves.
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
    /// An integer wider than two halves.
    Wider,
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
            Unlegalized::Wider => "an integer more than twice as wide as the backend's",
        })
    }
}

impl std::error::Error for Unlegal {}

/// The helper functions the pass adds to a unit, made once each when first called.
pub(super) struct Helpers {
    /// The id the next helper gets: after every function of the unit.
    next: FuncId,
    /// The unsigned division of pairs giving quotient and remainder, and where it was first
    /// needed.
    divide: Option<(FuncId, OriginId)>,
}

impl Helpers {
    /// The division helper, made when first called.
    pub(super) fn divide(&mut self, at: OriginId) -> FuncId {
        if let Some((id, _)) = self.divide {
            return id;
        }
        let id = self.next;
        self.next = FuncId::new(id.index() + 1);
        self.divide = Some((id, at));
        id
    }
}

impl Legalize {
    /// The integer split into halves: twice the widest.
    fn wide(self) -> Option<Scalar> {
        Scalar::int(self.widest.bits(0) * 2)
    }

    /// Whether a value of `ty` is an `f16` held as its bits.
    fn promoted(self, ty: Scalar) -> bool {
        ty == Scalar::F16 && self.half == Half::Promoted
    }

    /// The scalar a value of `ty`, not a wide integer, becomes.
    fn scalar(self, ty: Scalar) -> Scalar {
        if self.promoted(ty) { Scalar::I16 } else { ty }
    }

    fn half_bytes(self) -> u64 {
        self.widest.bytes(0)
    }

    /// The offsets of the low and the high half of a wide integer in memory.
    fn offsets(self) -> (u64, u64) {
        match self.endian {
            Endian::Little => (0, self.half_bytes()),
            Endian::Big => (self.half_bytes(), 0),
        }
    }

    /// `signature` with each wide integer passed as its halves, a wide result a record of them,
    /// and each promoted `f16` as its bits.
    fn signature(self, signature: &Signature, wide: Option<Scalar>) -> Result<Signature, Unlegalized> {
        let half = |t: &AbiType| match t {
            AbiType::Scalar(s) => self.promoted(*s),
            AbiType::Record(r) => r.fields.iter().any(|&(_, s)| self.promoted(s)),
            _ => false,
        };
        let passed = signature.params.iter().chain(&signature.result);
        if signature.conv == Conv::C && passed.clone().any(half) {
            return Err(Unlegalized::HalfAbi);
        }
        let promoted = |t: AbiType| match t {
            AbiType::Scalar(s) if self.promoted(s) => AbiType::Int(Scalar::I16, Ext::Zero),
            other => other,
        };
        let params = signature.params.iter().map(|p| match p {
            AbiType::Int(s, ext) if Some(*s) == wide => AbiType::Wide { half: self.widest, ext: *ext },
            other => promoted(other.clone()),
        });
        let result = signature.result.clone().map(|r| match r {
            AbiType::Int(s, _) if Some(s) == wide => {
                let (lo, hi) = self.offsets();
                let fields = vec![(lo.min(hi), self.widest), (lo.max(hi), self.widest)];
                AbiType::Record(Record { size: 2 * self.half_bytes(), align: self.half_bytes(), fields })
            }
            other => promoted(other),
        });
        Ok(Signature { conv: signature.conv, params: params.collect(), result })
    }
}

impl Unit {
    /// Splits every integer twice as wide as `legalize.widest` into halves; see the module.
    ///
    /// # Errors
    /// Returns the first operation on a wide integer that has no sequence over halves.
    pub fn legalize(&mut self, legalize: Legalize) -> Result<(), Unlegal> {
        let wide = legalize.wide();
        if wide.is_none() && legalize.half == Half::Computed {
            return Ok(());
        }
        let width = |s: Scalar| s.is_int() && s.bits(0) > legalize.widest.bits(0);
        for (_, f) in self.functions.iter() {
            if let Some((_, v)) = f.values.iter().find(|(_, v)| width(v.ty) && Some(v.ty) != wide) {
                let at = match v.def {
                    crate::func::Def::Inst(inst) => f.insts[inst].origin,
                    crate::func::Def::Param(block, _) => f.blocks[block].origin,
                };
                return Err(Unlegal { at, why: Unlegalized::Wider });
            }
        }
        let mut helpers = Helpers { next: FuncId::new(self.functions.len()), divide: None };
        let rewritten = self
            .functions
            .iter()
            .map(|(_, f)| Rewrite::function(self, f, legalize, wide, &mut helpers))
            .collect::<Result<Vec<_>, _>>()?;
        for (id, f) in rewritten.into_iter().enumerate() {
            self.functions[FuncId::new(id)] = f;
        }
        for (_, import) in self.imports.iter_mut() {
            if let ImportKind::Func(signature) = &import.kind {
                let signature =
                    legalize.signature(signature, wide).map_err(|why| Unlegal { at: import.origin, why })?;
                import.kind = ImportKind::Func(signature);
            }
        }
        if let Some((_, at)) = helpers.divide {
            self.functions.push(divide::helper(legalize, self.pointer_bits, at));
        }
        Ok(())
    }
}
