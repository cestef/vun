//! An instruction as emitted, and its text as disassemblers write it, from the form that encoded
//! it: aliases by their own mnemonics, immediates in hexadecimal.

use std::fmt;

use vun_isa::Via;
use vun_isa::imm::truncate;

use crate::operand::{Extend, Mem, Operand, Scale};
use crate::reg::Size;
use crate::table::{ARITH_BITS, Cond, Form, HALF_BITS, Imm, Kind, Op};

/// The most operands an instruction takes.
const MAX: usize = 4;

/// An instruction: its operation, its width and its operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inst<S> {
    pub op: Op,
    pub size: Size,
    operands: [Operand<S>; MAX],
    len: u8,
}

impl<S: Copy> Inst<S> {
    /// # Panics
    /// Panics past four operands, which no instruction takes.
    #[must_use]
    pub fn new(op: Op, size: Size, operands: &[Operand<S>]) -> Self {
        let mut all = [Operand::Imm(0); MAX];
        all[..operands.len()].copy_from_slice(operands);
        Self { op, size, operands: all, len: u8::try_from(operands.len()).expect("four operands at most") }
    }

    #[must_use]
    pub fn operands(&self) -> &[Operand<S>] {
        &self.operands[..self.len as usize]
    }

    /// The form encoding it.
    #[must_use]
    pub fn form(&self) -> Option<&'static Form> {
        self.op.form(self.size, self.operands())
    }
}

impl<S: Copy + fmt::Display> fmt::Display for Inst<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(form) = self.form() else { return write!(f, "{:?} (no form)", self.op) };
        let sized = form.at(self.size).expect("a form takes operands only at its sizes");
        write!(f, "{}{}", form.name.unwrap_or(self.op.mnemonic()), sized.suffix)?;
        let mut first = true;
        for (arg, operand) in form.args.iter().zip(self.operands()) {
            if let (Kind::Cond(Cond::Suffix), Operand::Cond(cc)) = (arg.kind, operand) {
                write!(f, ".{cc}")?;
                continue;
            }
            f.write_str(if first { " " } else { ", " })?;
            first = false;
            arg.kind.write(f, operand, self.size)?;
        }
        Ok(())
    }
}

/// An immediate as disassemblers write it: `#0x10`, `#-0x8`.
struct Hex(i64);

impl fmt::Display for Hex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "#{sign}{:#x}", self.0.unsigned_abs())
    }
}

impl Kind {
    /// Writes `operand`, which this kind takes, in an instruction of `size`.
    fn write<S: fmt::Display>(
        self,
        f: &mut fmt::Formatter<'_>,
        operand: &Operand<S>,
        size: Size,
    ) -> fmt::Result {
        match (self, operand) {
            (Self::Gpr(_, width), Operand::Reg(r)) => write!(f, "{}", r.name(width.at(size))),
            (Self::Fpr(width), Operand::V(v)) => write!(f, "{}", v.name(width.at(size))),
            (Self::Imm(imm), Operand::Imm(v)) => imm.write(f, *v, size.bits()),
            (Self::Shifted(_) | Self::Extended, Operand::Reg(r)) => write!(f, "{}", r.name(size)),
            (Self::Shifted(_), Operand::Shifted(r, shift)) => {
                write!(f, "{}, {} #{}", r.name(size), shift.kind, shift.amount)
            }
            (Self::Extended, Operand::Extended(r, extend, amount)) => {
                write!(f, "{}, {extend}", r.name(extend.reads()))?;
                if *amount > 0 { write!(f, " #{amount}") } else { Ok(()) }
            }
            (Self::Mem(_, width), Operand::Mem(mem)) => mem.write(f, width.at(size)),
            (_, Operand::Cond(cc)) => write!(f, "{cc}"),
            (_, Operand::Barrier(b)) => f.write_str(b.name()),
            (_, Operand::Sys(r)) => f.write_str(r.name()),
            (_, Operand::Label(label)) => write!(f, "{label}"),
            (_, Operand::Sym(target) | Operand::Page(target, Via::Direct)) => write!(f, "{target}"),
            (_, Operand::Page(target, Via::Got)) => write!(f, ":got:{target}"),
            (_, Operand::Page(target, Via::Descriptor)) => write!(f, ":tlsdesc:{target}"),
            (_, Operand::Lo12(target, Via::Descriptor)) => write!(f, ":tlsdesc_lo12:{target}"),
            (_, Operand::Lo12(target, _)) => write!(f, ":lo12:{target}"),
            _ => unreachable!("the form takes the operand"),
        }
    }
}

impl Imm {
    /// Writes `value` as this encoding holds it in an operation of `bits`.
    fn write(self, f: &mut fmt::Formatter<'_>, value: i64, bits: u32) -> fmt::Result {
        let v = truncate(value, bits);
        match self {
            Self::Arith if v >= 1 << ARITH_BITS => write!(f, "#{:#x}, lsl #{ARITH_BITS}", v >> ARITH_BITS),
            Self::Half if v != 0 => {
                let shift = v.trailing_zeros() / HALF_BITS * HALF_BITS;
                write!(f, "#{:#x}", v >> shift)?;
                if shift > 0 { write!(f, ", lsl #{shift}") } else { Ok(()) }
            }
            Self::Zero => f.write_str("#0.0"),
            _ => write!(f, "#{v:#x}"),
        }
    }
}

impl<S: fmt::Display> Mem<S> {
    /// Writes this operand of an access of `access`.
    fn write(&self, f: &mut fmt::Formatter<'_>, access: Size) -> fmt::Result {
        let base = self.base().name(Size::B64);
        match self {
            Mem::Off { offset: 0, .. } => write!(f, "[{base}]"),
            Mem::Off { offset, .. } => write!(f, "[{base}, {}]", Hex(*offset)),
            Mem::Pre { offset, .. } => write!(f, "[{base}, {}]!", Hex(*offset)),
            Mem::Post { offset, .. } => write!(f, "[{base}], {}", Hex(*offset)),
            Mem::Index { index, extend, scale, .. } => {
                write!(f, "[{base}, {}", index.name(extend.reads()))?;
                match (extend, scale) {
                    (&Extend::LSL, Scale::One) => {}
                    (&Extend::LSL, Scale::Access) => write!(f, ", lsl #{}", access.shift())?,
                    (_, Scale::One) => write!(f, ", {extend}")?,
                    (_, Scale::Access) => write!(f, ", {extend} #{}", access.shift())?,
                }
                f.write_str("]")
            }
            Mem::Lo12 { target, via: Via::Direct, .. } => write!(f, "[{base}, :lo12:{target}]"),
            Mem::Lo12 { target, via: Via::Got, .. } => write!(f, "[{base}, :got_lo12:{target}]"),
            Mem::Lo12 { target, via: Via::Descriptor, .. } => write!(f, "[{base}, :tlsdesc_lo12:{target}]"),
        }
    }
}
