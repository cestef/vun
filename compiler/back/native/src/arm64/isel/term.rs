//! Terminators: a branch jumps on the flags of the comparison it fuses, or tests its condition;
//! a switch compares case by case; a return passes the result where the convention returns it.
//! Leaving a block (`Cx::leave`) stores home what its targets read there, and each jump fills
//! its target's parameters (`Cx::jump`).

use vun_asm_arm64::{Cc, Emit, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::{Arch, Layout};
use vun_dwarf::Step;
use vun_lir::{AbiType, Block, Target, Terminator, Value};

use super::{Sel, narrow};
use crate::arm64::abi::{Carrier, Ret};
use crate::arm64::{Arm, FP, LR, Opnd, condition, constant};

/// The bytes the prologue pushed, popped by the epilogue.
const PAIR: i64 = 16;
/// The byte a boolean is.
const BYTE: i64 = 0xff;

impl Sel<'_, '_> {
    pub(super) fn terminate(&mut self, block: Block) -> Result<(), Lack> {
        let f = self.f;
        let term = &f.blocks[block].term;
        // A fused comparison sets the flags here; the stores and moves up to the jump keep them.
        let flags = match self.fused(block) {
            Some(fused) => {
                self.flags(fused.cond, fused.a, fused.b)?;
                self.used(fused.a);
                self.used(fused.b);
                Some(condition(fused.cond).expect("only comparisons on one condition fuse"))
            }
            None => None,
        };
        // The terminator's operand is read where it is now, before its uses are counted done; a
        // switch reads it extended to the width it compares at.
        let operand = match term {
            Terminator::Branch { cond: v, .. } if flags.is_none() => Some((self.reg(*v), Size::B32)),
            Terminator::Switch { value: v, .. } => {
                let (r, size) = (self.reg(*v), self.size(*v)?);
                if narrow(size) {
                    let extended = self.scratch();
                    self.asm.emit(Op::Uxt, size, &[extended.into(), r.into()]);
                    Some((extended, Size::B32))
                } else {
                    Some((r, size))
                }
            }
            _ => None,
        };
        let next = self.leave(block);
        match term {
            Terminator::Jump(target) => self.jump(target, next),
            Terminator::Branch { then, otherwise, .. } => {
                let holds = flags.unwrap_or_else(|| {
                    let (c, _) = operand.expect("a branch reads its condition");
                    self.asm.emit(Op::Tst, Size::B32, &[c.into(), Operand::Imm(BYTE)]);
                    Cc::Ne
                });
                let layout = Layout::of(then, otherwise, next);
                self.jump_if(if layout.inverted { holds.not() } else { holds }, layout.taken);
                self.jump(layout.fall, next);
            }
            Terminator::Switch { cases, default, .. } => {
                let (v, size) = operand.expect("a switch reads its value");
                for &(bits, ref target) in cases {
                    self.compare_case(v, size, bits);
                    self.jump_if(Cc::Eq, target);
                }
                self.jump(default, next);
            }
            Terminator::Return(value) => self.ret(*value)?,
            Terminator::Unreachable => self.asm.emit(Op::Udf, Size::B64, &[Operand::Imm(0)]),
        }
        self.left();
        Ok(())
    }

    /// Jumps to `target` where `holds` does, filling its parameters only on the way there.
    fn jump_if(&mut self, holds: Cc, target: &Target) {
        if target.args.is_empty() {
            let label = self.label(target.block);
            self.asm.emit(Op::Bcond, Size::B64, &[holds.into(), label.into()]);
        } else {
            let skip = self.asm.label();
            self.asm.emit(Op::Bcond, Size::B64, &[holds.not().into(), skip.into()]);
            self.jump(target, None);
            self.asm.bind(skip);
        }
    }

    /// `cmp v, bits`, at `size`: as an immediate where it fits one, else through the carrier.
    fn compare_case(&mut self, v: Reg, size: Size, bits: u128) {
        let value = u64::try_from(bits & u128::from(u64::MAX)).expect("masked").cast_signed();
        let operands: [Opnd; 2] = [v.into(), Operand::Imm(value)];
        if Op::Cmp.form(size, &operands).is_none() {
            constant(&mut self.asm, Arm::CARRIER, value.cast_unsigned());
            self.asm.emit(Op::Cmp, size, &[v.into(), Arm::CARRIER.into()]);
        } else {
            self.asm.emit(Op::Cmp, size, &operands);
        }
    }

    fn ret(&mut self, value: Option<Value>) -> Result<(), Lack> {
        let f = self.f;
        match (value, self.state.passing.ret.clone()) {
            (Some(v), Ret::Reg(carrier)) => {
                self.pass(v, carrier);
                if let (Some(AbiType::Int(scalar, ext)), Carrier::Int(r)) = (&f.signature.result, carrier) {
                    let size = self.size_of(*scalar)?;
                    self.extend(r, size, *ext);
                }
            }
            (_, Ret::Split(parts)) => {
                let at = self.state.gathered.expect("gathered where the entry receives the address");
                for part in parts {
                    let disp = i64::from(at) + i64::try_from(part.offset).expect("records fit memory");
                    self.load_part(part.carrier, FP, disp, part.bytes);
                }
            }
            _ => {}
        }
        self.restore();
        self.unwound(Step::Leaving);
        self.asm.emit(Op::Mov, Size::B64, &[Reg::Sp.into(), FP.into()]);
        let popped = vun_asm_arm64::Mem::Post { base: Reg::Sp, offset: PAIR };
        self.asm.emit(Op::Ldp, Size::B64, &[FP.into(), LR.into(), popped.into()]);
        self.unwound(Step::Popped);
        self.asm.emit(Op::Ret, Size::B64, &[]);
        self.unwound(Step::Returned);
        Ok(())
    }
}
