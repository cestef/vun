//! Where calls pass values: the System V AMD64 convention for `#[abi.c]`, and Vun's own, which
//! passes each LIR value of the signature in the same registers, then on the stack. Integers and
//! addresses travel in general registers, floats in SSE ones, and a small record's eightbytes in
//! either, by their class.

use std::collections::VecDeque;

use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Record, Scalar, Signature, Value};

use vun_asm_x86_64::{Reg, Xmm};

/// The registers integers and addresses are passed in, in order.
pub const ARGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9];
/// The registers a record of two eightbytes is returned in.
pub const RETURNS: [Reg; 2] = [Reg::Rax, Reg::Rdx];
/// The registers floats are passed in, in order.
pub const SSE_ARGS: [Xmm; 8] = [Xmm(0), Xmm(1), Xmm(2), Xmm(3), Xmm(4), Xmm(5), Xmm(6), Xmm(7)];
/// The registers a record's float eightbytes are returned in.
pub const SSE_RETURNS: [Xmm; 2] = [Xmm(0), Xmm(1)];
const EIGHTBYTE: u64 = 8;
/// The size up to which a record travels in registers.
const IN_REGISTERS: u64 = 16;

/// A register a value travels in: a general one, or an SSE one for floats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrier {
    Int(Reg),
    Sse(Xmm),
}

/// Where one LIR value of a signature travels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loc {
    Reg(Carrier),
    /// The address of a record whose eightbytes travel in registers, each with its byte count.
    Split(Vec<(Carrier, u64)>),
    /// At this offset of the arguments on the stack.
    Stack(u64),
    /// The address of a record copied onto the stack at this offset, of this size.
    Copied {
        offset: u64,
        size: u64,
    },
    /// The address a result in registers is written to: a place of the caller's.
    Returned,
}

/// How a result comes back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ret {
    None,
    /// A scalar in `rax`, or a float in `xmm0`.
    Reg(Carrier),
    /// A record's eightbytes in registers, each with its byte count.
    Split(Vec<(Carrier, u64)>),
    /// Written through the address passed first, which `rax` gives back.
    Hidden,
}

/// Where a call of a signature passes everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Passing {
    /// By LIR value of the signature (`Signature::values`).
    pub params: Vec<Loc>,
    pub ret: Ret,
    /// The bytes of arguments on the stack, a multiple of 16.
    pub stack: u64,
    /// How many SSE registers carry arguments: what a variadic C function reads from `al`.
    pub sse: u8,
}

/// The class of an eightbyte (System V, section 3.2.3): SSE when every field in it is a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Int,
    Sse,
}

/// The eightbytes of `record`: each one's class and how many bytes of it are the record's.
fn eightbytes(record: &Record) -> Result<Vec<(Class, u64)>, Missing> {
    let count = record.size.div_ceil(EIGHTBYTE);
    (0..count)
        .map(|i| {
            let (from, to) = (i * EIGHTBYTE, (i + 1) * EIGHTBYTE);
            let fields = record.fields.iter().filter(|&&(at, _)| at >= from && at < to);
            let classes = fields.map(|&(_, s)| Class::of(s)).collect::<Result<Vec<_>, _>>()?;
            let sse = !classes.is_empty() && classes.iter().all(|&c| c == Class::Sse);
            Ok((if sse { Class::Sse } else { Class::Int }, (record.size - from).min(EIGHTBYTE)))
        })
        .collect()
}

impl Class {
    fn of(scalar: Scalar) -> Result<Self, Missing> {
        match scalar {
            Scalar::F16 => Err(Missing::Half),
            Scalar::I128 => Err(Missing::Int128),
            Scalar::F32 | Scalar::F64 => Ok(Self::Sse),
            _ => Ok(Self::Int),
        }
    }
}

/// The argument registers still free, of each class, the next first.
struct Free {
    ints: VecDeque<Reg>,
    sses: VecDeque<Xmm>,
}

impl Free {
    /// Carriers for `classes`, all or none.
    fn take(&mut self, classes: &[Class]) -> Option<Vec<Carrier>> {
        let wanted = |class| classes.iter().filter(|&&c| c == class).count();
        if self.ints.len() < wanted(Class::Int) || self.sses.len() < wanted(Class::Sse) {
            return None;
        }
        Some(classes.iter().map(|&c| self.next(c).expect("counted")).collect())
    }

    fn next(&mut self, class: Class) -> Option<Carrier> {
        match class {
            Class::Int => self.ints.pop_front().map(Carrier::Int),
            Class::Sse => self.sses.pop_front().map(Carrier::Sse),
        }
    }
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
    ///
    /// # Panics
    /// Never: a record is split only over registers counted free.
    pub fn of(signature: &Signature) -> Result<Self, Missing> {
        let c = signature.conv == Conv::C;
        let ret = match &signature.result {
            Some(AbiType::Scalar(s)) => match Class::of(*s)? {
                Class::Sse => Ret::Reg(Carrier::Sse(SSE_RETURNS[0])),
                Class::Int => Ret::Reg(Carrier::Int(RETURNS[0])),
            },
            Some(AbiType::Int(..) | AbiType::Address(_)) => Ret::Reg(Carrier::Int(RETURNS[0])),
            Some(AbiType::Record(r)) if c && r.size <= IN_REGISTERS => {
                let (mut ints, mut sses) = (RETURNS.into_iter(), SSE_RETURNS.into_iter());
                let parts = eightbytes(r)?.into_iter().map(|(class, bytes)| {
                    let carrier = match class {
                        Class::Int => Carrier::Int(ints.next().expect("two eightbytes")),
                        Class::Sse => Carrier::Sse(sses.next().expect("two eightbytes")),
                    };
                    (carrier, bytes)
                });
                Ret::Split(parts.collect())
            }
            Some(AbiType::Record(_)) if c => Ret::Hidden,
            // Vun's convention passes the result's address as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let hidden = usize::from(ret == Ret::Hidden);
        let mut free =
            Free { ints: ARGS[hidden..].iter().copied().collect(), sses: SSE_ARGS.into_iter().collect() };
        let mut stack = 0;
        let mut on_stack = |size: u64| {
            let at = stack;
            stack += size.next_multiple_of(EIGHTBYTE);
            at
        };
        let mut params = Vec::new();
        for param in &signature.params {
            let loc = match param {
                AbiType::Record(r) if c => {
                    let parts = eightbytes(r)?;
                    let classes: Vec<Class> = parts.iter().map(|&(class, _)| class).collect();
                    match free.take(&classes).filter(|_| r.size <= IN_REGISTERS) {
                        Some(carriers) => {
                            Loc::Split(carriers.into_iter().zip(parts.iter().map(|&(_, n)| n)).collect())
                        }
                        None => Loc::Copied { offset: on_stack(r.size), size: r.size },
                    }
                }
                AbiType::Scalar(s) => {
                    let class = Class::of(*s)?;
                    free.next(class).map_or_else(|| Loc::Stack(on_stack(EIGHTBYTE)), Loc::Reg)
                }
                AbiType::Int(..) | AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => {
                    free.next(Class::Int).map_or_else(|| Loc::Stack(on_stack(EIGHTBYTE)), Loc::Reg)
                }
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match ret {
                Ret::Hidden => Loc::Reg(Carrier::Int(ARGS[0])),
                Ret::Split(_) => Loc::Returned,
                _ => free.next(Class::Int).map_or_else(|| Loc::Stack(on_stack(EIGHTBYTE)), Loc::Reg),
            });
        }
        let sse = u8::try_from(SSE_ARGS.len() - free.sses.len()).expect("eight registers");
        Ok(Self { params, ret, stack: stack.next_multiple_of(2 * EIGHTBYTE), sse })
    }
}
