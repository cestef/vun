//! An instruction as emitted, and its text in the unified syntax assemblers read: the mnemonic, its
//! condition and width suffixes, `.w` on a 32-bit form of a mnemonic with 16-bit ones, and the
//! operands.

use std::fmt;

use vun_isa::imm::truncate;

use crate::operand::{List, Mem, Operand};
use crate::reg::Size;
use crate::table::{Encoding, Form, Imm, Kind, Op};

/// The most operands an instruction takes: `mrc`'s six.
const MAX: usize = 6;

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
    /// Panics past six operands, which no instruction takes.
    #[must_use]
    pub fn new(op: Op, size: Size, operands: &[Operand<S>]) -> Self {
        let mut all = [Operand::Imm(0); MAX];
        all[..operands.len()].copy_from_slice(operands);
        Self { op, size, operands: all, len: u8::try_from(operands.len()).expect("six operands at most") }
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

    /// Its text as encoded by `form`.
    #[must_use]
    pub fn by(&self, form: &'static Form) -> Text<'_, S> {
        Text { inst: self, form }
    }
}

/// An instruction's text as a given form encodes it.
pub struct Text<'i, S> {
    inst: &'i Inst<S>,
    form: &'static Form,
}

impl<S: Copy + fmt::Display> fmt::Display for Text<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (inst, form) = (self.inst, self.form);
        let sized = form.at(inst.size).expect("a form takes operands only at its sizes");
        f.write_str(form.name.unwrap_or(inst.op.mnemonic()))?;
        for (arg, operand) in form.args.iter().zip(inst.operands()) {
            if let (Kind::Cond, Operand::Cond(cc)) = (arg.kind, operand) {
                write!(f, "{cc}")?;
            }
        }
        f.write_str(sized.suffix)?;
        if form.name.is_none() && inst.op.qualified(form) {
            write!(f, ".{}", Encoding::W.name())?;
        }
        for qualifier in sized.qualifiers {
            write!(f, ".{qualifier}")?;
        }
        let mut first = true;
        for (arg, operand) in form.args.iter().zip(inst.operands()) {
            if arg.kind == Kind::Cond {
                continue;
            }
            f.write_str(if first { " " } else { ", " })?;
            first = false;
            arg.kind.write(f, operand, inst.size)?;
        }
        Ok(())
    }
}

impl<S: Copy + fmt::Display> fmt::Display for Inst<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.form() {
            Some(form) => self.by(form).fmt(f),
            None => write!(f, "{:?} (no form)", self.op),
        }
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
            (Self::Fpr(width, _), Operand::V(v)) => write!(f, "{}", v.name_at(width.at(size))),
            (Self::Imm(Imm::Modified | Imm::Half), Operand::Imm(v)) => {
                write!(f, "#{:#x}", truncate(*v, u32::BITS))
            }
            (_, Operand::Imm(v)) => write!(f, "#{v}"),
            (_, Operand::Reg(r)) => write!(f, "{r}"),
            (_, Operand::Shifted(r, shift)) => write!(f, "{r}, {} #{}", shift.kind, shift.amount),
            (_, Operand::Mem(mem)) => write!(f, "{mem}"),
            (_, Operand::Cond(cc)) => write!(f, "{cc}"),
            (_, Operand::Label(label)) => write!(f, "{label}"),
            (_, Operand::Sym(target)) => write!(f, "{target}"),
            (_, Operand::Low(target)) => write!(f, "#:lower16:{target}"),
            (_, Operand::High(target)) => write!(f, "#:upper16:{target}"),
            (_, Operand::List(list)) => write!(f, "{list}"),
            (_, Operand::Special(s)) => f.write_str(s.name()),
            (_, Operand::Status(s)) => f.write_str(s.name()),
            (_, Operand::Mask(m)) => f.write_str(m.name()),
            (_, Operand::Barrier(b)) => f.write_str(b.name()),
            (_, Operand::Coprocessor(p)) => f.write_str(p.name()),
            (_, Operand::Cr(c)) => f.write_str(c.name()),
            (_, Operand::V(_)) => unreachable!("the form takes the operand"),
        }
    }
}

impl fmt::Display for Mem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Off { base, offset: 0 } => write!(f, "[{base}]"),
            Self::Off { base, offset } => write!(f, "[{base}, #{offset}]"),
            Self::Pre { base, offset } => write!(f, "[{base}, #{offset}]!"),
            Self::Post { base, offset } => write!(f, "[{base}], #{offset}"),
            Self::Index { base, index, shift: 0 } => write!(f, "[{base}, {index}]"),
            Self::Index { base, index, shift } => write!(f, "[{base}, {index}, lsl #{shift}]"),
        }
    }
}

impl fmt::Display for List {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Regs(regs) => {
                let names = crate::Reg::ALL.iter().filter(|r| regs & 1 << r.number() != 0);
                let names: Vec<_> = names.map(|r| r.name()).collect();
                write!(f, "{{{}}}", names.join(", "))
            }
            Self::Doubles { first, count: 1 } => write!(f, "{{{}}}", first.name_at(Size::B64)),
            Self::Doubles { first, count } => {
                let last = crate::V::ALL[usize::from(first.number()) + 2 * usize::from(count - 1)];
                write!(f, "{{{}-{}}}", first.name_at(Size::B64), last.name_at(Size::B64))
            }
        }
    }
}
