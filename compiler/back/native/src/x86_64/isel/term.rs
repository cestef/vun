//! Terminators: a branch jumps on the flags of the comparison it fuses, or tests its condition;
//! a switch compares case by case; a return passes the result where the convention returns it.
//! Leaving a block (`Cx::leave`) stores home what its targets read there, and each jump fills
//! its target's parameters (`Cx::jump`).

use vun_asm_x86_64::{Cc, Emit, Mem, Op, Operand, Reg, Size};
use vun_backend::Lack;
use vun_codegen::{Arch, Layout};
use vun_dwarf::Step;
use vun_lir::{AbiType, Address, Block, Target, Terminator, Value};

use super::Sel;
use crate::x86_64::abi::{Carrier, Ret};
use crate::x86_64::snippets::CMP;
use crate::x86_64::{X86, condition};

impl Sel<'_, '_> {
    pub(super) fn terminate(&mut self, block: Block) -> Result<(), Lack> {
        let f = self.f;
        let term = &f.blocks[block].term;
        // A fused comparison sets the flags here; the stores and moves up to the jump keep them.
        let flags = match self.fused(block) {
            Some(fused) => {
                let size = self.size(fused.a)?;
                self.apply(CMP, &[fused.a, fused.b], None, size, None);
                self.used(fused.a);
                self.used(fused.b);
                Some(condition(fused.cond).expect("only integer comparisons fuse"))
            }
            None => None,
        };
        // The terminator's operand is read where it is now, before its uses are counted done.
        let operand = match term {
            Terminator::Branch { cond: v, .. } | Terminator::Switch { value: v, .. } if flags.is_none() => {
                Some((self.reg(*v), self.size(*v)?))
            }
            _ => None,
        };
        let next = self.leave(block);
        match term {
            Terminator::Jump(target) => self.jump(target, next),
            Terminator::Branch { then, otherwise, .. } => {
                let holds = flags.unwrap_or_else(|| {
                    let (c, _) = operand.expect("a branch reads its condition");
                    self.asm.emit(Op::Test, Size::B8, &[c.into(), c.into()]);
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
                    self.jump_if(Cc::E, target);
                }
                self.jump(default, next);
            }
            Terminator::Return(value) => self.ret(*value)?,
            Terminator::Unreachable => self.asm.emit(Op::Ud2, Size::B64, &[]),
        }
        self.left();
        Ok(())
    }

    /// Jumps to `target` where `holds` does, filling its parameters only on the way there.
    fn jump_if(&mut self, holds: Cc, target: &Target) {
        if target.args.is_empty() {
            let label = self.label(target.block);
            self.asm.emit(Op::J, Size::B64, &[holds.into(), label.into()]);
        } else {
            let skip = self.asm.label();
            self.asm.emit(Op::J, Size::B64, &[holds.not().into(), skip.into()]);
            self.jump(target, None);
            self.asm.bind(skip);
        }
    }

    /// `cmp v, bits`, at `size`: as an immediate where it fits one, else through the carrier.
    fn compare_case(&mut self, v: Reg, size: Size, bits: u128) {
        let value = vun_asm::imm::sign_extend(vun_asm::imm::low64(bits), size.bits());
        let operands: [Operand<Address>; 2] = [v.into(), Operand::Imm(value)];
        if Op::Cmp.form(size, &operands).is_none() {
            self.asm.emit(Op::Mov, Size::B64, &[X86::CARRIER.into(), Operand::Imm(value)]);
            self.asm.emit(Op::Cmp, size, &[v.into(), X86::CARRIER.into()]);
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
                for (k, &(carrier, _)) in parts.iter().enumerate() {
                    let offset = at + crate::width::offset_of(k as u64 * X86::WORD);
                    self.load_part(carrier, Mem::at(Reg::Rbp, offset), X86::WORD);
                }
            }
            (_, Ret::Hidden) => {
                let home = self.state.hidden.expect("kept where the entry receives it");
                self.asm.emit(Op::Mov, Size::B64, &[Reg::Rax.into(), Mem::at(Reg::Rbp, home).into()]);
            }
            _ => {}
        }
        self.restore();
        self.unwound(Step::Leaving);
        self.asm.emit(Op::Leave, Size::B64, &[]);
        self.unwound(Step::Popped);
        self.asm.emit(Op::Ret, Size::B64, &[]);
        self.unwound(Step::Returned);
        Ok(())
    }
}
