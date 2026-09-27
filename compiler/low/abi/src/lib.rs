//! Calling conventions as data, and the one classifier every backend that places arguments reads
//! (spec 13, section 8.5): where a call passes each LIR value of a signature, and where its result
//! comes back.
//!
//! A [`Convention`] is a convention's rules, which this crate declares, and the registers they
//! name, which each backend declares for its machine; [`Passing::of`] applies it. Placements name
//! the backend's own registers, of any type, so this crate knows no assembler.

use std::error::Error;
use std::fmt;

mod classify;
mod convention;
mod free;
mod record;

pub use convention::{Convention, Large, Pair, Records, Registers, Rules, Short, Stack, Variadic};

/// A register a value travels in: a general one, or a floating-point one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Carrier<I, F> {
    Int(I),
    Float(F),
}

/// Bytes of a record travelling in one register: its register, their offset in the record and
/// their count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Part<R> {
    pub reg: R,
    pub offset: u64,
    pub bytes: u64,
}

/// Where one LIR value of a signature travels, `R` naming registers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Loc<R> {
    Reg(R),
    /// At this offset of the arguments on the stack, taking these bytes.
    Stack {
        offset: u64,
        bytes: u64,
    },
    /// The address of a record whose parts travel in registers.
    Split(Vec<Part<R>>),
    /// The address of a record copied whole onto the stack at this offset.
    Copied {
        offset: u64,
        size: u64,
    },
    /// The address of a record of this size, copied by the caller, its address passed where `at`
    /// says.
    Indirect {
        size: u64,
        at: Box<Loc<R>>,
    },
    /// The address a result returned in registers is written to: a place of the caller's.
    Returned,
}

/// How a result comes back.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ret<R> {
    None,
    /// A scalar, in the first result register of its class.
    Reg(R),
    /// A record's parts in result registers.
    Split(Vec<Part<R>>),
    /// Written through the address the caller passes: in the register the convention gives it
    /// ([`Registers::result_address`]), else in the first general argument register.
    Memory,
}

/// Where a call of a signature passes everything, `R` naming registers.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Passing<R> {
    /// By LIR value of the signature (`Signature::values`), the address of a result in memory
    /// last.
    pub params: Vec<Loc<R>>,
    pub ret: Ret<R>,
    /// The bytes of arguments on the stack, a multiple of 16.
    pub stack: u64,
    /// How many floating-point registers carry arguments, which a variadic call states under
    /// some conventions ([`Variadic::FloatCount`]).
    pub floats: u8,
}

impl<I: Copy, F> Passing<Carrier<I, F>> {
    /// The arguments of `args`, a value per LIR value of the signature, passed in general
    /// registers, with their registers.
    pub fn in_ints<T: Copy>(&self, args: &[T]) -> Vec<(T, I)> {
        let locs = args.iter().zip(&self.params);
        locs.filter_map(|(&a, loc)| match loc {
            Loc::Reg(Carrier::Int(r)) => Some((a, *r)),
            _ => None,
        })
        .collect()
    }
}

/// What a convention cannot pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpassable {
    /// A half-precision float, which C passes by no convention built.
    Half,
    /// A 128-bit integer as one value, which legalization splits first.
    Int128,
    /// A record's eightbyte of floats holding this many bytes, neither one `f32` nor two nor a
    /// `f64`: a layout only an explicit size makes.
    FloatPart(u64),
    /// A function without a prologue, which no convention passes anything to.
    Naked,
}

impl fmt::Display for Unpassable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Half => f.write_str("`f16` values"),
            Self::Int128 => f.write_str("128-bit integers as one value"),
            Self::FloatPart(bytes) => write!(f, "a record part of {bytes} bytes of floats"),
            Self::Naked => f.write_str("naked functions"),
        }
    }
}

impl Error for Unpassable {}

#[cfg(test)]
mod tests;
