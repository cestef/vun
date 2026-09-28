//! Terminators: a branch jumps on the flags of the comparison it fuses, or tests its condition;
//! a switch compares case by case; a return passes the result where the convention returns it
//! and pops the return address into the program counter. Each jump runs the plan's moves for its
//! edge (`Cx::jump`), a conditional one on its own way to its target.

use vun_asm::imm::sign_extend;
use vun_asm_thumb::{Cc, Emit, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::snippet::Args;
use vun_codegen::{Arch, Layout};
use vun_dwarf::Step;
use vun_lir::{AbiType, Block, Target, Terminator, Value};

use super::Sel;
use super::entry::RETURN;
use crate::abi::{Carrier, Ret};
use crate::asm::AsmExt;
use crate::{FAR, FP, Flags, Instr, Opnd, Thumb2};

/// The byte a boolean is.
const BYTE: i64 = 0xff;

impl Sel<'_, '_> {
    pub(super) fn terminate(&mut self, block: Block) -> Result<(), Lack> {
        let f = self.f;
        let term = &f.blocks[block].term;
        // A fused comparison sets the flags here; the stores and moves up to the jump keep them.
        let flags = match self.fused(block) {
            Some(fused) => {
                let (snippet, size) = self.look().flags(fused.cond, fused.a)?;
                self.snippet(snippet, &Args { ins: &[fused.a, fused.b], out: None, size, cond: None });
                Some(fused.cond.holds().expect("only comparisons on one condition fuse"))
            }
            None => None,
        };
        // A switch reads its value extended to the word it compares at.
        let operand = match term {
            Terminator::Branch { cond: v, .. } if flags.is_none() => Some(self.reg(*v)),
            Terminator::Switch { value: v, .. } => {
                let (r, size) = (self.reg(*v), self.look().size(*v)?);
                if size.narrow() {
                    let extended = self.scratch();
                    let (op, size) = Instr::Zext.at(size);
                    self.asm.emit(op, size, &[extended.into(), r.into()]);
                    Some(extended)
                } else {
                    Some(r)
                }
            }
            _ => None,
        };
        let next = self.next(block);
        match term {
            Terminator::Jump(target) => self.jump(target, next),
            Terminator::Branch { then, otherwise, .. } => {
                let holds = flags.unwrap_or_else(|| {
                    let c = operand.expect("a branch reads its condition");
                    self.asm.emit(Op::Tst, Size::B32, &[c.into(), Operand::Imm(BYTE)]);
                    Cc::Ne
                });
                let layout = Layout::of(then, otherwise, next);
                self.jump_if(if layout.inverted { holds.not() } else { holds }, layout.taken);
                self.jump(layout.fall, next);
            }
            Terminator::Switch { cases, default, .. } => {
                let v = operand.expect("a switch reads its value");
                for &(bits, ref target) in cases {
                    self.compare_case(v, bits);
                    self.jump_if(Cc::Eq, target);
                }
                self.jump(default, next);
            }
            Terminator::Return(value) => self.ret(*value)?,
            Terminator::Unreachable => self.asm.emit(Op::Udf, Size::B32, &[Operand::Imm(0)]),
        }
        Ok(())
    }

    /// Jumps to `target` where `holds` does, running the moves of its edge only on the way there.
    fn jump_if(&mut self, holds: Cc, target: &Target) {
        if self.moves_on(target) {
            let skip = self.asm.label();
            self.asm.emit(Op::Bcond, Size::B32, &[holds.not().into(), skip.into()]);
            self.jump(target, None);
            self.asm.bind(skip);
        } else {
            let label = self.label(target.block);
            self.asm.emit(Op::Bcond, Size::B32, &[holds.into(), label.into()]);
        }
    }

    /// `cmp v, bits`: as an immediate where it fits one, else through the carrier.
    fn compare_case(&mut self, v: Reg, bits: u128) {
        let value = vun_asm::imm::low64(bits);
        let operands: [Opnd; 2] = [v.into(), Operand::Imm(sign_extend(value, u32::BITS))];
        if Op::Cmp.form(Size::B32, &operands).is_none() {
            self.asm.constant(Thumb2::CARRIER, value);
            self.asm.emit(Op::Cmp, Size::B32, &[v.into(), Thumb2::CARRIER.into()]);
        } else {
            self.asm.emit(Op::Cmp, Size::B32, &operands);
        }
    }

    fn ret(&mut self, value: Option<Value>) -> Result<(), Lack> {
        let f = self.f;
        match (value, self.state.passing.ret.clone()) {
            (Some(v), Ret::Reg(carrier)) => {
                // A value kept in a general register is there already, the plan having put it.
                if matches!(carrier, Carrier::Float(_)) || self.recomputed(v).is_some() {
                    self.pass(v, carrier);
                }
                if let (Some(AbiType::Int(scalar, ext)), Carrier::Int(r)) = (&f.signature.result, carrier) {
                    let size = self.size_of(*scalar)?;
                    self.extend(r, size, *ext);
                }
            }
            (_, Ret::Split(parts)) => {
                let at = self.state.gathered.expect("gathered where the entry receives the address");
                for part in parts {
                    let disp = i64::from(at) + i64::try_from(part.offset).expect("records fit memory");
                    self.load_part(part.reg, FP, disp, part.bytes);
                }
            }
            _ => {}
        }
        let far = self.state.far_at;
        Thumb2::load(&mut self.asm, FAR, far);
        self.restore();
        self.unwound(Step::Leaving);
        self.asm.emit(Op::Mov, Size::B32, &[Reg::Sp.into(), FP.into()]);
        self.asm.emit(Op::Pop, Size::B32, &[Operand::List(RETURN)]);
        self.unwound(Step::Popped);
        self.unwound(Step::Returned);
        Ok(())
    }
}
