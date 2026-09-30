//! Variadic arguments as clang's wasm32 convention passes them: the caller writes each into a
//! buffer in its frame, at the argument's alignment and never less than a word, and passes the
//! buffer's address after the fixed arguments. C's promotions are kept: an integer narrower than
//! a word is written as a word, a `float` as a `double`; a record, or anything passed in memory,
//! is written as the address of a copy the caller makes.

use vun_backend::Missing;
use vun_lir::{AbiType, Ext, Scalar, Value};

use super::func::Lower;
use crate::module::op::{Num, Op};

/// The least alignment of an argument in the buffer: a word.
const WORD: u64 = 4;
/// The bytes of the `double` a `float` is promoted to.
const DOUBLE: u64 = 8;

/// How one LIR value of the variadic arguments is written into the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Spill {
    /// As a scalar of this type, a narrower integer first extended as it says.
    Scalar { ty: Scalar, narrow: Option<(u32, Ext)> },
    /// A `float`, written as a `double`.
    Promoted,
    /// The address of a copy of these bytes.
    Copy { size: u64, align: u64 },
}

/// Where each LIR value of the variadic arguments goes in the buffer, and the buffer's size and
/// alignment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Varargs {
    pub(super) spills: Vec<(u64, Spill)>,
    pub(super) size: u64,
    pub(super) align: u64,
}

impl Varargs {
    pub(super) fn of(params: &[AbiType]) -> Result<Self, Missing> {
        let mut varargs = Self { spills: Vec::new(), size: 0, align: WORD };
        for p in params {
            match *p {
                AbiType::Scalar(Scalar::F16) => return Err(Missing::Half),
                AbiType::Scalar(Scalar::F32) => varargs.place(DOUBLE, DOUBLE, Spill::Promoted),
                AbiType::Scalar(ty) => {
                    let bytes = u64::from(ty.bits(0) / 8);
                    varargs.place(bytes, bytes, Spill::Scalar { ty, narrow: None });
                }
                AbiType::Int(ty, ext) => {
                    let bytes = u64::from(ty.bits(0) / 8).max(WORD);
                    let narrow = super::abi::narrow(ty).map(|bits| (bits, ext));
                    // A narrower integer is written as a word.
                    let held = if bytes == WORD { Scalar::I32 } else { ty };
                    varargs.place(bytes, bytes, Spill::Scalar { ty: held, narrow });
                }
                AbiType::Address(_) => {
                    varargs.place(WORD, WORD, Spill::Scalar { ty: Scalar::Ptr, narrow: None });
                }
                AbiType::Wide { limb, limbs, .. } => {
                    // The whole integer is aligned to its size, its limbs in order, low first.
                    let bytes = u64::from(limb.bits(0) / 8);
                    let whole = bytes * u64::from(limbs);
                    varargs.size = varargs.size.next_multiple_of(whole);
                    varargs.align = varargs.align.max(whole);
                    for _ in 0..limbs {
                        varargs.place(bytes, bytes, Spill::Scalar { ty: limb, narrow: None });
                    }
                }
                AbiType::Record(ref r) => {
                    varargs.place(WORD, WORD, Spill::Copy { size: r.size, align: r.align });
                }
                AbiType::Memory { size, align } => varargs.place(WORD, WORD, Spill::Copy { size, align }),
            }
        }
        Ok(varargs)
    }

    /// Places a value of `bytes` at `align`, at least a word.
    fn place(&mut self, bytes: u64, align: u64, spill: Spill) {
        let align = align.max(WORD);
        let at = self.size.next_multiple_of(align);
        self.spills.push((at, spill));
        self.size = at + bytes.max(WORD);
        self.align = self.align.max(align);
    }
}

impl Lower<'_> {
    /// Writes `args` into a buffer as `varargs` places them, and pushes its address.
    pub(super) fn spill(&mut self, args: &[Value], varargs: &Varargs) -> Result<(), Missing> {
        let buffer = self.alloc(varargs.size, varargs.align);
        for (&arg, &(offset, spill)) in args.iter().zip(&varargs.spills) {
            self.frame_addr(buffer + offset);
            match spill {
                Spill::Scalar { ty, narrow } => {
                    self.get(arg);
                    if let Some((bits, Ext::Sign)) = narrow {
                        self.extend(bits);
                    }
                    self.op(Op::Store(Self::mem(ty)?));
                }
                Spill::Promoted => {
                    self.get(arg);
                    self.op(Op::Num(Num::F64PromoteF32));
                    self.op(Op::Store(Self::mem(Scalar::F64)?));
                }
                Spill::Copy { size, align } => {
                    let copy = self.alloc(size, align);
                    self.frame_addr(copy);
                    self.get(arg);
                    self.op(Op::I32Const(Self::i32(size)));
                    self.op(Op::MemoryCopy);
                    self.frame_addr(copy);
                    self.op(Op::Store(Self::mem(Scalar::Ptr)?));
                }
            }
        }
        self.frame_addr(buffer);
        Ok(())
    }
}
