//! Terminators: every value still used is stored home, then each jump fills its target's
//! parameters' homes by the sequenced moves of `Moves`, and a jump to the block emitted next
//! falls through.

use vun_backend::Lack;
use vun_lir::analysis::{Location, Moves};
use vun_lir::{Block, Target, Terminator};

use super::super::abi::{Carrier, Ret};
use super::super::encode::Alu;
use super::super::{Cc, Mem, Reg, Rm, Size};
use super::FnCompiler;

impl FnCompiler<'_> {
    pub(super) fn terminate(&mut self, block: Block) -> Result<(), Lack> {
        let term = self.f.blocks[block].term.clone();
        // A fused comparison sets the flags here; the stores and moves up to the jump keep them.
        let flags = match self.fused(block) {
            Some((_, cond, a, b)) => {
                let cc = self.compare(cond, a, b)?;
                self.release(&[a, b], None);
                Some(cc)
            }
            None => None,
        };
        // The terminator's operands are read from where they are now; nothing after it uses
        // them but through their homes.
        let operand = match &term {
            _ if flags.is_some() => None,
            Terminator::Branch { cond: v, .. } | Terminator::Switch { value: v, .. } => Some(self.get(*v)),
            _ => None,
        };
        let size = match &term {
            Terminator::Switch { value, .. } => Some(self.size(*value)?),
            _ => None,
        };
        for v in term.operands() {
            self.left[v] = self.left[v].saturating_sub(1);
        }
        // A block emitted next that only this one reaches starts with the registers as they are
        // here; only a target entered otherwise needs every value home.
        let next = self.order.next(block);
        let falls = |t: &Target| next == Some(t.block) && self.cfg.preds[t.block].len() == 1;
        let (keep, only) = match &term {
            Terminator::Jump(t) => (falls(t), falls(t)),
            Terminator::Branch { otherwise, .. } => (falls(otherwise), false),
            _ => (false, false),
        };
        if !only {
            self.spill_live();
        }
        match &term {
            Terminator::Jump(target) => self.jump(target, next),
            Terminator::Branch { then, otherwise, .. } => {
                let holds = flags.unwrap_or_else(|| {
                    let c = operand.expect("a branch reads its condition");
                    self.asm.test(Size::B8, c, c);
                    Cc::Ne
                });
                if then.args.is_empty() {
                    self.asm.jcc(holds, self.labels[then.block]);
                } else {
                    let skip = self.asm.label();
                    self.asm.jcc(holds.not(), skip);
                    self.jump(then, None);
                    self.asm.bind(skip);
                }
                self.jump(otherwise, next);
            }
            Terminator::Switch { cases, default, .. } => {
                let (v, size) = (operand.expect("a switch reads its value"), size.expect("sized above"));
                for (bits, target) in cases {
                    self.compare_case(v, size, *bits);
                    if target.args.is_empty() {
                        self.asm.jcc(Cc::E, self.labels[target.block]);
                    } else {
                        let skip = self.asm.label();
                        self.asm.jcc(Cc::Ne, skip);
                        self.jump(target, None);
                        self.asm.bind(skip);
                    }
                }
                self.jump(default, next);
            }
            Terminator::Return(value) => self.ret(*value)?,
            Terminator::Unreachable => self.asm.ud2(),
        }
        self.locked.clear();
        if keep {
            self.kept = next;
        } else {
            self.clear();
        }
        Ok(())
    }

    /// `cmp v, bits`, at `size`.
    fn compare_case(&mut self, v: Reg, size: Size, bits: u128) {
        let shift = 128 - size.bytes() * 8;
        let signed = (bits << shift).cast_signed() >> shift;
        if let Ok(imm) = i32::try_from(signed) {
            self.asm.alu_imm(Alu::Cmp, size, Rm::Reg(v), imm);
        } else {
            self.asm.mov_ri(Reg::SCRATCH, u64::try_from(bits & u128::from(u64::MAX)).expect("masked"));
            self.asm.alu(Alu::Cmp, size, v, Reg::SCRATCH);
        }
    }

    /// Fills the target's parameters, then jumps unless it is `next`.
    fn jump(&mut self, target: &Target, next: Option<Block>) {
        for m in Moves::of(self.f, target).0 {
            let carrier = match m.to {
                Location::Temp(_) => Reg::TEMP,
                Location::Value(_) => Reg::SCRATCH,
            };
            let from = match m.from {
                Location::Value(v) => {
                    self.get_into(v, carrier);
                    carrier
                }
                Location::Temp(_) => Reg::TEMP,
            };
            if let Location::Value(p) = m.to {
                if let Some(r) = self.fixed[p] {
                    self.asm.mov_rr(r, from);
                } else {
                    let home = self.home(p);
                    self.asm.store(Size::B64, home, from);
                }
            }
        }
        if next != Some(target.block) {
            self.asm.jmp(self.labels[target.block]);
        }
    }

    fn ret(&mut self, value: Option<vun_lir::Value>) -> Result<(), Lack> {
        if let (Some(v), Ret::Reg(carrier)) = (value, self.abi.ret.clone()) {
            let bytes = u64::from(self.size(v)?.bytes());
            self.pass(v, carrier, bytes);
            if let (Some(vun_lir::AbiType::Int(scalar, ext)), Carrier::Int(r)) =
                (self.f.signature.result.clone(), carrier)
            {
                let size = self.size_of(scalar)?;
                self.extend_int(r, size, ext);
            }
        }
        match &self.abi.ret {
            Ret::Split(parts) => {
                let at = self.gathered.expect("gathered where the entry receives the address");
                for (k, &(carrier, _)) in parts.clone().iter().enumerate() {
                    let offset = at + i32::try_from(k * 8).expect("two eightbytes");
                    self.load_part(carrier, Mem::at(Reg::Rbp, offset), 8);
                }
            }
            Ret::Hidden => {
                let home = self.hidden.expect("kept where the entry receives it");
                self.asm.load(Size::B64, Reg::Rax, Mem::at(Reg::Rbp, home));
            }
            Ret::Reg(_) | Ret::None => {}
        }
        self.restore();
        self.asm.epilogue();
        Ok(())
    }
}
