//! Where calls pass values: the System V AMD64 convention for `#[abi.c]`, and Vun's own, which
//! passes each LIR value of the signature in the same registers, then on the stack.

use vun_backend::Missing;
use vun_lir::{AbiType, Conv, Record, Signature};

use super::Reg;

/// The registers integers and addresses are passed in, in order.
pub const ARGS: [Reg; 6] = [Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9];
/// The registers a record of two eightbytes is returned in.
pub const RETURNS: [Reg; 2] = [Reg::Rax, Reg::Rdx];
const EIGHTBYTE: u64 = 8;
/// The size up to which a record of integers travels in registers.
const IN_REGISTERS: u64 = 16;

/// Where one LIR value of a signature travels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loc {
    Reg(Reg),
    /// The address of a record whose eightbytes travel in registers, each with its byte count.
    Split(Vec<(Reg, u64)>),
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
    /// A scalar in `rax`.
    Reg,
    /// A record's eightbytes in `rax` and `rdx`, each with its byte count.
    Split(Vec<(Reg, u64)>),
    /// Written through the address passed first, which `rax` gives back.
    Hidden,
}

/// Where a call of a signature passes everything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// By LIR value of the signature (`Signature::values`).
    pub params: Vec<Loc>,
    pub ret: Ret,
    /// The bytes of arguments on the stack, a multiple of 16.
    pub stack: u64,
}

/// The eightbytes of a record of `size` bytes: how many bytes of each are its own.
fn eightbytes(size: u64) -> Vec<u64> {
    (0..size.div_ceil(EIGHTBYTE)).map(|i| (size - i * EIGHTBYTE).min(EIGHTBYTE)).collect()
}

/// A record passes in registers when small enough; floats are not passed yet.
fn in_registers(record: &Record) -> Result<bool, Missing> {
    if record.fields.iter().any(|(_, s)| s.is_float()) {
        return Err(Missing::Float);
    }
    Ok(record.size <= IN_REGISTERS)
}

impl Frame {
    /// Where `signature` passes its values.
    ///
    /// # Errors
    /// Returns what the backend cannot pass yet: floats.
    ///
    /// # Panics
    /// Never: a record is split only over registers counted free.
    pub fn of(signature: &Signature) -> Result<Self, Missing> {
        let ret = match &signature.result {
            Some(AbiType::Scalar(_)) => return Err(Missing::Float),
            Some(AbiType::Int(..) | AbiType::Address(_)) => Ret::Reg,
            Some(AbiType::Record(r)) if signature.conv == Conv::C && in_registers(r)? => {
                Ret::Split(RETURNS.into_iter().zip(eightbytes(r.size)).collect())
            }
            Some(AbiType::Record(_)) if signature.conv == Conv::C => Ret::Hidden,
            // Vun's convention passes the result's address as its last value.
            None | Some(AbiType::Record(_) | AbiType::Memory { .. }) => Ret::None,
        };
        let mut regs = ARGS.iter().copied().skip(usize::from(ret == Ret::Hidden)).peekable();
        let mut stack = 0;
        let mut on_stack = |size: u64| {
            let at = stack;
            stack += size.next_multiple_of(EIGHTBYTE);
            at
        };
        let mut params = Vec::new();
        for param in &signature.params {
            let loc = match param {
                AbiType::Scalar(_) => return Err(Missing::Float),
                AbiType::Record(r) if signature.conv == Conv::C => {
                    let parts = eightbytes(r.size);
                    let fits = in_registers(r)? && regs.len() >= parts.len();
                    if fits {
                        Loc::Split(parts.into_iter().map(|n| (regs.next().expect("counted"), n)).collect())
                    } else {
                        Loc::Copied { offset: on_stack(r.size), size: r.size }
                    }
                }
                AbiType::Int(..) | AbiType::Address(_) | AbiType::Record(_) | AbiType::Memory { .. } => {
                    regs.next().map_or_else(|| Loc::Stack(on_stack(EIGHTBYTE)), Loc::Reg)
                }
            };
            params.push(loc);
        }
        if signature.through() {
            params.push(match ret {
                Ret::Hidden => Loc::Reg(ARGS[0]),
                Ret::Split(_) => Loc::Returned,
                _ => regs.next().map_or_else(|| Loc::Stack(on_stack(EIGHTBYTE)), Loc::Reg),
            });
        }
        Ok(Self { params, ret, stack: stack.next_multiple_of(2 * EIGHTBYTE) })
    }
}
