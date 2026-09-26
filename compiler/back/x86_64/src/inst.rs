//! An instruction as emitted, and its text in Intel syntax, from the form that encoded it.

use std::fmt;

use vun_asm::imm::{sign_extend, truncate};

use crate::operand::{Base, Mem, Operand};
use crate::reg::Size;
use crate::table::{Form, Op};

/// The most operands an instruction takes.
const MAX: usize = 3;

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
    /// Panics past three operands, which no instruction takes.
    #[must_use]
    pub fn new(op: Op, size: Size, operands: &[Operand<S>]) -> Self {
        let mut all = [Operand::Imm(0); MAX];
        all[..operands.len()].copy_from_slice(operands);
        Self { op, size, operands: all, len: u8::try_from(operands.len()).expect("three operands at most") }
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
        f.write_str(form.name.unwrap_or(self.op.mnemonic()))?;
        let mut first = true;
        for (&kind, operand) in form.kinds.iter().zip(self.operands()) {
            if let Operand::Cond(cc) = operand {
                f.write_str(cc.suffix())?;
                continue;
            }
            f.write_str(if first { " " } else { ", " })?;
            first = false;
            match operand {
                Operand::Reg(r) => f.write_str(r.name(kind.width(self.size).unwrap_or(Size::B64)))?,
                Operand::Xmm(x) => write!(f, "{x}")?,
                Operand::Mem(mem) => {
                    if let Some(width) = kind.width(self.size) {
                        write!(f, "{} ", width.keyword())?;
                    }
                    write_mem(f, mem)?;
                }
                // The value the encoding holds, as a signed number of its width.
                Operand::Imm(v) => {
                    let bits = kind.imm_bytes(self.size).unwrap_or(8) * 8;
                    let value = sign_extend(truncate(*v, bits), bits);
                    if value < 0 {
                        write!(f, "-{:#x}", value.unsigned_abs())?;
                    } else {
                        write!(f, "{value:#x}")?;
                    }
                }
                Operand::Label(l) => write!(f, "{l}")?,
                Operand::Sym(s) => write!(f, "{s}")?,
                Operand::Cond(_) => unreachable!("written as the mnemonic's suffix"),
            }
        }
        Ok(())
    }
}

fn write_mem<S: fmt::Display>(f: &mut fmt::Formatter<'_>, mem: &Mem<S>) -> fmt::Result {
    match &mem.base {
        Base::Reg(r) => write!(f, "[{}", r.name(Size::B64))?,
        Base::Rip(target, via) => {
            write!(f, "[rip + {target}{}", if *via == vun_asm::Via::Got { "@got" } else { "" })?;
        }
    }
    match mem.disp {
        0 => {}
        d if d < 0 => write!(f, " - {:#x}", d.unsigned_abs())?,
        d => write!(f, " + {d:#x}")?,
    }
    f.write_str("]")
}
