//! Where calls pass values: the procedure call standard for arm64 (AAPCS64) for `#[abi.c]`, and
//! Vun's own, which passes each LIR value of the signature in the same registers, then on the
//! stack. Integers and addresses travel in general registers, floats in floating-point ones; a
//! record of up to four floats of one type (a homogeneous aggregate) travels a field per
//! floating-point register, another record of up to 16 bytes in up to two general registers, and
//! a larger one as the address of a copy the caller makes. Apple's variant (`Convention::Darwin`)
//! gives a C call's scalars on the stack their own size at their own alignment, not a word each.

use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Signature, Value};

use vun_asm_arm64::{Reg, V};

use free::Free;
use shape::{Shape, is_float};

mod free;
mod shape;

/// The registers integers and addresses are passed in, in order, and a result returned in.
pub const ARGS: [Reg; 8] = [Reg::X0, Reg::X1, Reg::X2, Reg::X3, Reg::X4, Reg::X5, Reg::X6, Reg::X7];
/// The registers floats are passed in, in order, and a result returned in.
pub const FP_ARGS: [V; 8] = [V(0), V(1), V(2), V(3), V(4), V(5), V(6), V(7)];
/// The register the address a result is written through is passed in.
pub const INDIRECT: Reg = Reg::X8;
const WORD: u64 = 8;
/// The most a composite on the stack is aligned to.
const COMPOSITE_ALIGN: u64 = 16;
const POINTER_BITS: u32 = 64;
/// The size up to which a record travels in general registers.
const IN_REGISTERS: u64 = 16;
/// The most fields of a homogeneous aggregate.
const HOMOGENEOUS: usize = 4;

/// A register a value travels in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrier {
    Int(Reg),
    Fp(V),
}

/// Bytes of a record travelling in one register: its carrier, their offset and their count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub carrier: Carrier,
    pub offset: u64,
    pub bytes: u64,
}

/// The variant of AAPCS64 a target follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Convention {
    #[default]
    Aapcs64,
    /// Apple's, on macOS and iOS.
    Darwin,
}

/// Where one LIR value of a signature travels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loc {
    Reg(Carrier),
    /// At this offset of the arguments on the stack, taking these bytes.
    Stack {
        offset: u64,
        bytes: u64,
    },
    /// The address of a record whose parts travel in registers.
    Split(Vec<Part>),
    /// The address of a record copied onto the stack at this offset, of this size.
    Copied {
        offset: u64,
        size: u64,
    },
    /// The address of a record of this size, copied by the caller, its address passed where
    /// `at` says.
    Indirect {
        size: u64,
        at: Box<Loc>,
    },
    /// The address a result in registers is written to: a place of the caller's.
    Returned,
}

/// How a result comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ret {
    None,
    Reg(Carrier),
    /// A record's parts in registers.
    Split(Vec<Part>),
    /// Written through the address passed in `x8`.
    Indirect,
}

/// Where a call of a signature passes everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passing {
    /// By LIR value of the signature (`Signature::values`).
    pub params: Vec<Loc>,
    pub ret: Ret,
    /// The bytes of arguments on the stack, a multiple of 16.
    pub stack: u64,
}

impl Passing {
    /// The arguments of `args` passed in general registers, with their registers.
    #[must_use]
    pub fn in_regs(&self, args: &[Value]) -> Vec<(Value, Reg)> {
        let locs = args.iter().zip(&self.params);
        locs.filter_map(|(&a, loc)| match loc {
            Loc::Reg(Carrier::Int(r)) => Some((a, *r)),
            _ => None,
        })
        .collect()
    }

    /// Where `signature` passes its values.
    ///
    /// # Errors
    /// Returns what the backend cannot pass yet: `f16` and 128-bit integers.
    pub fn of(signature: &Signature, convention: Convention) -> Result<Self, Missing> {
        let c = signature.conv == Conv::C;
        let ret = match &signature.result {
            Some(AbiType::Scalar(s)) => {
                Ret::Reg(if is_float(*s)? { Carrier::Fp(FP_ARGS[0]) } else { Carrier::Int(ARGS[0]) })
            }
            Some(AbiType::Int(..) | AbiType::Address(_)) => Ret::Reg(Carrier::Int(ARGS[0])),
            Some(AbiType::Record(r)) if c => {
                let (mut ints, mut fps) = (&ARGS[..], &FP_ARGS[..]);
                Shape::of(r)?.parts(r, &mut ints, &mut fps).map_or(Ret::Indirect, Ret::Split)
            }
            Some(AbiType::Wide { .. }) => unreachable!("the verifier refuses a wide result"),
            // Vun's convention passes the result's address as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let packed = c && convention == Convention::Darwin;
        let mut free = Free { ints: &ARGS, fps: &FP_ARGS, stack: 0, packed };
        let mut params = Vec::new();
        for param in &signature.params {
            let loc = match param {
                AbiType::Wide { .. } => {
                    params.extend(free.pair());
                    continue;
                }
                AbiType::Record(r) if c => {
                    let shape = Shape::of(r)?;
                    match shape.parts(r, &mut free.ints, &mut free.fps) {
                        Some(parts) => Loc::Split(parts),
                        None if matches!(shape, Shape::Large) => {
                            Loc::Indirect { size: r.size, at: Box::new(free.word(false, WORD)) }
                        }
                        // Too few registers left: none of the class is used after it.
                        None => {
                            match shape {
                                Shape::Homogeneous { .. } => free.fps = &[],
                                _ => free.ints = &[],
                            }
                            Loc::Copied { offset: free.stack(r.size, r.align), size: r.size }
                        }
                    }
                }
                AbiType::Scalar(s) => free.word(is_float(*s)?, s.bytes(POINTER_BITS)),
                AbiType::Int(s, _) => free.word(false, s.bytes(POINTER_BITS)),
                AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => free.word(false, WORD),
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match ret {
                Ret::Indirect => Loc::Reg(Carrier::Int(INDIRECT)),
                Ret::Split(_) => Loc::Returned,
                _ => free.word(false, WORD),
            });
        }
        Ok(Self { params, ret, stack: free.stack.next_multiple_of(2 * WORD) })
    }
}
