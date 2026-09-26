//! Where yaxpeax writes an instruction otherwise than Arm does, the fields it decoded are compared
//! instead of its text, for these cases only:
//! - an `orr` of a bitmask from the zero register is written as the `mov` of its value even where
//!   Arm prefers `movz` or `movn` for it (`MoveWidePreferred`), which the table writes `mov` too;
//! - a 32-bit register zero-extended by a 64-bit `add` or `sub` (`uxtw`) is decoded as unshifted;
//! - a register index keeps its shift amount but not its `S` bit: the index of a byte access,
//!   scaled by 1, loses its `#0`, and an unscaled `sxtw` or `sxtx` gains one.

use yaxpeax_arm::armv8::a64::{Instruction, Operand as Decoded, ShiftStyle, SizeCode};

use super::Emitted;
use crate::{Extend, Inst, Kind, Mem, Op, Operand, Reg, Scale, Size};

impl Emitted {
    /// Whether `decoded`, written `theirs` unlike any of `ours` (this word's texts, aliases
    /// included), holds what was encoded, in one of the cases yaxpeax writes otherwise.
    pub(super) fn known_quirk(&self, decoded: &Instruction, theirs: &str, ours: &[String]) -> bool {
        match self.operands.as_slice() {
            [rd, Operand::Reg(Reg::Zr), imm @ Operand::Imm(_)] if self.op == Op::Orr => {
                theirs == Inst::new(Op::Mov, self.size, &[*rd, *imm]).to_string()
            }
            [.., Operand::Extended(r, Extend::Uxtw, amount)] if self.size == Size::B64 => {
                let read = Decoded::RegShift(ShiftStyle::LSL, *amount, SizeCode::W, r.number().into());
                decoded.operands.contains(&read) && ours.iter().any(|t| theirs == t.replace(", uxtw", ""))
            }
            [.., Operand::Mem(Mem::Index { base, index, extend, scale })] => {
                let access = match self.form.args.last().map(|a| a.kind) {
                    Some(Kind::Mem(_, width)) => width.at(self.size),
                    _ => unreachable!("an index is memory"),
                };
                let size = if extend.reads() == Size::B64 { SizeCode::X } else { SizeCode::W };
                let style = match extend {
                    Extend::Uxtw => ShiftStyle::UXTW,
                    Extend::Sxtw => ShiftStyle::SXTW,
                    Extend::Sxtx => ShiftStyle::SXTX,
                    _ => ShiftStyle::LSL,
                };
                let amount = if *scale == Scale::Access { u8::try_from(access.shift()).unwrap() } else { 0 };
                let read =
                    Decoded::RegRegOffset(base.number().into(), index.number().into(), size, style, amount);
                let before = |text: &str| text.split('[').next().map(str::to_owned);
                decoded.operands.contains(&read) && ours.iter().any(|t| before(theirs) == before(t))
            }
            _ => false,
        }
    }
}
