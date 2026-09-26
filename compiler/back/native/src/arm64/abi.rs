//! Where calls pass values: the procedure call standard for arm64 (AAPCS64) for `#[abi.c]`, and
//! Vun's own, which passes each LIR value of the signature in the same registers, then on the
//! stack. Integers and addresses travel in general registers, floats in floating-point ones; a
//! record of up to four floats of one type (a homogeneous aggregate) travels a field per
//! floating-point register, another record of up to 16 bytes in up to two general registers, and
//! a larger one as the address of a copy the caller makes.

use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Record, Scalar, Signature};

use vun_asm_arm64::{Reg, V};

/// The registers integers and addresses are passed in, in order, and a result returned in.
pub const ARGS: [Reg; 8] = [Reg::X0, Reg::X1, Reg::X2, Reg::X3, Reg::X4, Reg::X5, Reg::X6, Reg::X7];
/// The registers floats are passed in, in order, and a result returned in.
pub const FP_ARGS: [V; 8] = [V(0), V(1), V(2), V(3), V(4), V(5), V(6), V(7)];
/// The register the address a result is written through is passed in.
pub const INDIRECT: Reg = Reg::X8;
const WORD: u64 = 8;
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

/// Where one LIR value of a signature travels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loc {
    Reg(Carrier),
    /// At this offset of the arguments on the stack.
    Stack(u64),
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

/// How a record travels, by its fields.
enum Shape {
    /// Fields of one float type, each in a floating-point register.
    Homogeneous { count: usize, bytes: u64 },
    /// In general registers, a word each.
    Words(usize),
    /// Too large for registers: through a copy's address.
    Large,
}

impl Shape {
    fn of(record: &Record) -> Result<Self, Missing> {
        let floats = record.fields.iter().map(|&(_, s)| is_float(s)).collect::<Result<Vec<_>, _>>()?;
        if let Some(&(_, first)) = record.fields.first()
            && floats.iter().all(|&f| f)
            && record.fields.len() <= HOMOGENEOUS
        {
            let bytes = if first == Scalar::F32 { 4 } else { 8 };
            let laid_out =
                record.fields.iter().enumerate().all(|(k, &(at, s))| s == first && at == k as u64 * bytes);
            if laid_out && record.size == record.fields.len() as u64 * bytes {
                return Ok(Self::Homogeneous { count: record.fields.len(), bytes });
            }
        }
        Ok(if record.size <= IN_REGISTERS {
            Self::Words(usize::try_from(record.size.div_ceil(WORD)).expect("two words"))
        } else {
            Self::Large
        })
    }

    /// Its parts, over `ints` and `fps`, the registers taken; `None` if too few are left.
    fn parts(&self, record: &Record, ints: &mut &[Reg], fps: &mut &[V]) -> Option<Vec<Part>> {
        match *self {
            Self::Homogeneous { count, bytes } => {
                let (taken, rest) = fps.split_at_checked(count)?;
                *fps = rest;
                let parts = taken.iter().enumerate();
                Some(
                    parts
                        .map(|(k, &v)| Part { carrier: Carrier::Fp(v), offset: k as u64 * bytes, bytes })
                        .collect(),
                )
            }
            Self::Words(count) => {
                let (taken, rest) = ints.split_at_checked(count)?;
                *ints = rest;
                let parts = taken.iter().enumerate().map(|(k, &r)| {
                    let offset = k as u64 * WORD;
                    Part { carrier: Carrier::Int(r), offset, bytes: (record.size - offset).min(WORD) }
                });
                Some(parts.collect())
            }
            Self::Large => None,
        }
    }
}

/// A scalar's carrier class: whether it travels in a floating-point register.
fn is_float(scalar: Scalar) -> Result<bool, Missing> {
    match scalar {
        Scalar::F16 => Err(Missing::Half),
        Scalar::I128 => Err(Missing::Int128),
        Scalar::F32 | Scalar::F64 => Ok(true),
        _ => Ok(false),
    }
}

/// The argument registers still free and the stack taken, as the call standard counts them.
struct Free {
    ints: &'static [Reg],
    fps: &'static [V],
    stack: u64,
}

impl Free {
    fn stack(&mut self, size: u64) -> u64 {
        let at = self.stack;
        self.stack += size.next_multiple_of(WORD);
        at
    }

    fn word(&mut self, float: bool) -> Loc {
        let next = if float {
            self.fps.split_first().map(|(&v, rest)| {
                self.fps = rest;
                Carrier::Fp(v)
            })
        } else {
            self.ints.split_first().map(|(&r, rest)| {
                self.ints = rest;
                Carrier::Int(r)
            })
        };
        next.map_or_else(|| Loc::Stack(self.stack(WORD)), Loc::Reg)
    }
}

impl Passing {
    /// Where `signature` passes its values.
    ///
    /// # Errors
    /// Returns what the backend cannot pass yet: `f16` and 128-bit integers.
    pub fn of(signature: &Signature) -> Result<Self, Missing> {
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
            // Vun's convention passes the result's address as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let mut free = Free { ints: &ARGS, fps: &FP_ARGS, stack: 0 };
        let mut params = Vec::new();
        for param in &signature.params {
            let loc = match param {
                AbiType::Record(r) if c => {
                    let shape = Shape::of(r)?;
                    match shape.parts(r, &mut free.ints, &mut free.fps) {
                        Some(parts) => Loc::Split(parts),
                        None if matches!(shape, Shape::Large) => {
                            Loc::Indirect { size: r.size, at: Box::new(free.word(false)) }
                        }
                        // Too few registers left: none of the class is used after it.
                        None => {
                            match shape {
                                Shape::Homogeneous { .. } => free.fps = &[],
                                _ => free.ints = &[],
                            }
                            Loc::Copied { offset: free.stack(r.size), size: r.size }
                        }
                    }
                }
                AbiType::Scalar(s) => free.word(is_float(*s)?),
                AbiType::Int(..) | AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => {
                    free.word(false)
                }
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match ret {
                Ret::Indirect => Loc::Reg(Carrier::Int(INDIRECT)),
                Ret::Split(_) => Loc::Returned,
                _ => free.word(false),
            });
        }
        Ok(Self { params, ret, stack: free.stack.next_multiple_of(2 * WORD) })
    }
}
