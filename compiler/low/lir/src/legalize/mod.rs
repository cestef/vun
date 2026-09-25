//! Legalizing integers too wide for a backend (spec 13, section 8.1): each value of twice the
//! widest width a backend computes with becomes a pair of halves, low first, and every operation
//! on it a sequence over the halves, before the backend reads the unit. A wide parameter is
//! passed as its two halves; a wide result is a record of them, which the C convention returns
//! in registers as C returns its 128-bit integers, and Vun's through memory. Division calls a
//! helper function the pass adds. Afterwards no value is wider than the widest width.

mod divide;
mod halves;
mod rewrite;
#[cfg(test)]
mod tests;

use std::fmt;

use vun_index::Idx;
use vun_origin::OriginId;
use vun_target::Endian;

use crate::func::Record;
use crate::{AbiType, Ext, FuncId, ImportKind, Scalar, Signature, Unit};
use rewrite::Rewrite;

/// How a unit is legalized for a backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Legalize {
    /// The widest integer the backend computes with.
    pub widest: Scalar,
    /// Where the halves of a wide integer are in memory.
    pub endian: Endian,
}

/// Why an operation on a wide integer has no sequence over halves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unlegalized {
    /// An atomic access of a wide integer.
    Atomic,
    /// A conversion between a wide integer and a float.
    Float,
    /// An integer wider than two halves.
    Wider,
}

/// An operation the pass cannot legalize, where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unlegal {
    pub at: OriginId,
    pub why: Unlegalized,
}

impl fmt::Display for Unlegal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.why {
            Unlegalized::Atomic => "an atomic access of an integer wider than the backend's",
            Unlegalized::Float => "a conversion between a float and an integer wider than the backend's",
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

    /// `signature` with each wide integer passed as its halves and a wide result as a record of
    /// them.
    fn signature(self, signature: &Signature, wide: Scalar) -> Signature {
        let params = signature.params.iter().flat_map(|p| match p {
            AbiType::Int(s, ext) if *s == wide => {
                vec![AbiType::Int(self.widest, Ext::Zero), AbiType::Int(self.widest, *ext)]
            }
            other => vec![other.clone()],
        });
        let result = signature.result.clone().map(|r| match r {
            AbiType::Int(s, _) if s == wide => {
                let (lo, hi) = self.offsets();
                let fields = vec![(lo.min(hi), self.widest), (lo.max(hi), self.widest)];
                AbiType::Record(Record { size: 2 * self.half_bytes(), align: self.half_bytes(), fields })
            }
            other => other,
        });
        Signature { conv: signature.conv, params: params.collect(), result }
    }
}

impl Unit {
    /// Splits every integer twice as wide as `legalize.widest` into halves; see the module.
    ///
    /// # Errors
    /// Returns the first operation on a wide integer that has no sequence over halves.
    pub fn legalize(&mut self, legalize: Legalize) -> Result<(), Unlegal> {
        let Some(wide) = legalize.wide() else { return Ok(()) };
        let width = |s: Scalar| s.is_int() && s.bits(0) > legalize.widest.bits(0);
        for (_, f) in self.functions.iter() {
            if let Some((_, v)) = f.values.iter().find(|(_, v)| width(v.ty) && v.ty != wide) {
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
                import.kind = ImportKind::Func(legalize.signature(signature, wide));
            }
        }
        if let Some((_, at)) = helpers.divide {
            self.functions.push(divide::helper(legalize, self.pointer_bits, at));
        }
        Ok(())
    }
}
