//! Calling conventions as data, and the one classifier every backend that places arguments reads
//! (spec 13, section 8.5): where a call passes each LIR value of a signature, and where its result
//! comes back.
//!
//! A [`Convention`] is a convention's rules, which this crate declares, and the registers they
//! name, which each backend declares for its machine; [`Passing::of`] applies it. Placements name
//! the backend's own registers, of any type, so this crate knows no assembler.

use std::error::Error;
use std::fmt;

mod bank;
mod classify;
mod convention;
mod free;
mod record;
mod rules;

pub use convention::{Convention, Registers};
pub use rules::{Floats, Large, Pair, Records, Rules, Short, Split, Stack, Variadic};

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

/// The bytes of a record from `from` to its end, on the stack at `offset` of the arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rest {
    pub from: u64,
    pub offset: u64,
    pub bytes: u64,
}

impl<R> Part<R> {
    /// The bytes of a record from its start to the end of the last of `parts` and `rest`.
    pub fn extent(parts: &[Self], rest: Option<&Rest>) -> u64 {
        let ends = parts.iter().map(|p| p.offset + p.bytes).chain(rest.map(|r| r.from + r.bytes));
        ends.max().unwrap_or(0)
    }
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
    /// The address of a record whose first parts travel in registers, and the rest, if any, on
    /// the stack (AAPCS, rule C.5).
    Split {
        parts: Vec<Part<R>>,
        rest: Option<Rest>,
    },
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
    /// The bytes of arguments on the stack, a multiple of the stack's alignment.
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
    /// A scalar of these bytes as one value, wider than a register of its class, which
    /// legalization splits first: a 128-bit integer, or a 64-bit one or a soft `f64` on a
    /// 32-bit machine.
    Wider(u64),
    /// A record's eightbyte of floats holding this many bytes, neither one `f32` nor two nor a
    /// `f64`: a layout only an explicit size makes.
    FloatPart(u64),
    /// A function without a prologue, which no convention passes anything to.
    Naked,
    /// An integer of these bytes, which C has no type of on the machine.
    Integer(u64),
}

impl fmt::Display for Unpassable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Half => f.write_str("`f16` values"),
            Self::Wider(bytes) => write!(f, "a value of {bytes} bytes in one register"),
            Self::FloatPart(bytes) => write!(f, "a record part of {bytes} bytes of floats"),
            Self::Naked => f.write_str("naked functions"),
            Self::Integer(bytes) => write!(f, "a C integer of {bytes} bytes"),
        }
    }
}

impl Error for Unpassable {}

#[cfg(test)]
mod tests;
