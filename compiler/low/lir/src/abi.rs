//! The types a signature passes and how a function is called.

use std::iter;

use vun_codec::{Decode, Encode};
use vun_target::Convention;

use crate::ty::Scalar;

/// How a function is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Conv {
    /// Vun's own: scalars as they are; aggregates by address, a result through an address
    /// passed first. Backends choose registers freely.
    Vun,
    /// The target's C calling convention (`#[abi.c]`): records are passed and returned by value,
    /// as C passes structs.
    C,
    /// No prologue or epilogue (`#[abi.naked]`): the body is its own entry.
    Naked,
}

/// A value a signature passes: a scalar; a record passed by value under the C convention; or a
/// value in memory passed by address under Vun's. The body and callers hold both in memory,
/// reaching them by address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum AbiType {
    /// A float.
    Scalar(Scalar),
    /// An address, and whether anything else reaches what it does while the call runs.
    Address(Alias),
    /// An integer, and how it extends to a wider register, as its range reads it: C declares it
    /// signed or unsigned by that.
    Int(Scalar, Ext),
    /// An integer of `limbs` integers of type `limb`, wider than any the backend computes with,
    /// passed as its limbs, low first, which the backend's call convention places together;
    /// `ext` is how the whole extends.
    Wide {
        limb: Scalar,
        limbs: u8,
        ext: Ext,
    },
    Record(Record),
    Memory {
        size: u64,
        align: u64,
    },
}

/// What else reaches what an address passed to a call reaches, while the call runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Alias {
    /// Anything may.
    Any,
    /// Nothing but this address and those made from it: an exclusive borrow (spec 05, section 8).
    Exclusive,
}

/// How an integer extends to a wider register: with zeros, or copies of its sign bit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Ext {
    Zero,
    Sign,
}

impl Ext {
    /// The bits of the integer `bits` of type `from`, extended this way to 64 bits.
    #[must_use]
    pub fn widen(self, from: Scalar, bits: u128) -> u128 {
        match self {
            Self::Zero => bits,
            Self::Sign => Scalar::I64.truncate(from.signed(bits)),
        }
    }
}

/// A record as C lays it out: its size, alignment and the scalars at their offsets.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Record {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<(u64, Scalar)>,
}

impl AbiType {
    /// The types of the LIR values standing for it: the scalar, the limbs of a wide integer,
    /// or the address of the record.
    pub fn values(&self) -> impl Iterator<Item = Scalar> + use<> {
        let (scalar, count) = match self {
            Self::Scalar(s) | Self::Int(s, _) => (*s, 1),
            Self::Wide { limb, limbs, .. } => (*limb, usize::from(*limbs)),
            Self::Address(_) | Self::Record(_) | Self::Memory { .. } => (Scalar::Ptr, 1),
        };
        iter::repeat_n(scalar, count)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Signature {
    pub conv: Conv,
    pub params: Vec<AbiType>,
    /// The result; one in memory is written through the address the body receives after its
    /// parameters, and callers pass.
    pub result: Option<AbiType>,
    /// Under the C convention, the parameter the variadic arguments start at (C's `...`); the
    /// function is an import declared at the arity its callers use.
    pub variadic: Option<usize>,
    /// Under the C convention, the one it names (`#[abi.stdcall]`) where it is not the target's.
    pub convention: Option<Convention>,
}

impl Signature {
    /// The types of the values the entry block receives and a call passes: the parameters, then
    /// the address of a result in memory.
    #[must_use]
    pub fn values(&self) -> Vec<Scalar> {
        let out = self.through().then_some(Scalar::Ptr);
        self.params.iter().flat_map(AbiType::values).chain(out).collect()
    }

    /// The parameter each LIR value of [`Signature::values`] stands for, but the address of a
    /// result in memory: a wide integer's twice.
    pub fn passed(&self) -> impl Iterator<Item = &AbiType> {
        self.params.iter().flat_map(|p| iter::repeat_n(p, p.values().count()))
    }

    /// The parameters passed in memory under Vun's convention, by the position of their address
    /// among [`Signature::values`], with their size and alignment.
    pub fn memory(&self) -> impl Iterator<Item = (usize, u64, u64)> {
        let mut at = 0;
        self.params.iter().filter_map(move |param| {
            let found = match *param {
                AbiType::Memory { size, align } => Some((at, size, align)),
                _ => None,
            };
            at += param.values().count();
            found
        })
    }

    /// Whether every integer it passes says how it extends, and every address what aliases it.
    #[must_use]
    pub fn well_formed(&self) -> bool {
        self.params.iter().chain(&self.result).all(|p| match p {
            AbiType::Scalar(s) => s.is_float(),
            AbiType::Int(s, _) | AbiType::Wide { limb: s, .. } => s.is_int(),
            AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => true,
        })
    }

    /// Whether it marks variadic arguments only as C passes them: under the C convention, from
    /// one of its parameters.
    #[must_use]
    pub fn variadic_well_formed(&self) -> bool {
        self.variadic.is_none_or(|fixed| self.conv == Conv::C && fixed <= self.params.len())
    }

    /// Whether the result is written through an address rather than returned as one value: a
    /// record, memory, or a wide integer's halves, which the convention may return in registers
    /// the caller then writes there.
    #[must_use]
    pub fn through(&self) -> bool {
        matches!(self.result, Some(AbiType::Record(_) | AbiType::Wide { .. } | AbiType::Memory { .. }))
    }

    /// The type of the value a call gives, for a scalar result.
    #[must_use]
    pub fn returns(&self) -> Option<Scalar> {
        match &self.result {
            Some(AbiType::Scalar(s) | AbiType::Int(s, _)) => Some(*s),
            Some(AbiType::Address(_)) => Some(Scalar::Ptr),
            Some(AbiType::Wide { .. } | AbiType::Record(_) | AbiType::Memory { .. }) | None => None,
        }
    }
}
