//! The frame and the parameters: the prologue saves the frame pointer and the link register and
//! reserves the frame, its size built in `x16` by two moves written when the function ends;
//! parameters passed in memory are addresses recomputed where used, the others are received
//! where the convention passes them once the callee-saved registers are chosen.

use crate::width::offset_of;
use vun_asm_arm64::{Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::{Arch, Place};
use vun_dwarf::Step;

use super::Sel;
use crate::arm64::abi::{Carrier, Loc, Ret};
use crate::arm64::{Arm, FP, LR};

/// Where the caller's stack arguments start, above the saved frame pointer and link register.
pub(super) const CALLER_ARGS: i32 = 16;
/// The bytes the prologue pushes: the frame pointer and the link register.
const PAIR: i64 = 16;
/// The bits of the frame size each of its two moves writes.
const HALF: u32 = 16;
/// The register the frame size is built in.
const SIZE: Reg = Arm::CARRIER;

impl Sel<'_, '_> {
    /// `stp x29, x30, [sp, #-16]!; mov x29, sp; mov x16, size; sub sp, sp, x16`, and where each
    /// parameter passed in memory is.
    pub(super) fn prologue(&mut self) {
        let pushed = Mem::Pre { base: Reg::Sp, offset: -PAIR };
        self.asm.emit(Op::Stp, Size::B64, &[FP.into(), LR.into(), pushed.into()]);
        self.unwound(Step::Pushed);
        self.asm.emit(Op::Mov, Size::B64, &[FP.into(), Reg::Sp.into()]);
        self.unwound(Step::Framed);
        self.state.frame_at = self.asm.here();
        frame_size(&mut self.asm, 0);
        self.asm.emit(Op::Sub, Size::B64, &[Reg::Sp.into(), Reg::Sp.into(), SIZE.into()]);
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        for (i, &v) in f.blocks[entry].params.iter().enumerate() {
            let place = match self.state.passing.params[i].clone() {
                Loc::Copied { offset, .. } => Place::frame(CALLER_ARGS + offset_of(offset)),
                Loc::Split(parts) => Place::frame(self.buffer(&parts)),
                Loc::Returned => {
                    let Ret::Split(parts) = self.state.passing.ret.clone() else { continue };
                    let buffer = self.buffer(&parts);
                    self.state.gathered = Some(buffer);
                    Place::frame(buffer)
                }
                Loc::Reg(_) | Loc::Stack { .. } | Loc::Indirect { .. } => continue,
            };
            self.recompute_as(v, place);
        }
    }

    /// Writes the frame's size into the prologue's moves.
    pub(super) fn finish(&mut self, frame: u64) {
        let mut moves = vun_asm_arm64::Asm::listed();
        frame_size(&mut moves, frame);
        let at = self.state.frame_at;
        self.asm.replace(at, moves.finish().expect("no labels"));
    }

    /// Receives the parameters passed in registers and on the stack.
    pub(super) fn entry(&mut self) {
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        let params = &f.blocks[entry].params;
        let locs = self.state.passing.params.clone();
        // Values in general registers are received first, so that one received from a
        // floating-point register, which takes a general register, overwrites none still to be
        // received.
        let fp = |loc: &Loc| matches!(loc, Loc::Reg(Carrier::Float(_)));
        let ordered = params
            .iter()
            .zip(&locs)
            .filter(|(_, l)| !fp(l))
            .chain(params.iter().zip(&locs).filter(|(_, l)| fp(l)));
        for (&v, loc) in ordered {
            // A record copied by the caller is received as its address.
            let loc = match loc {
                Loc::Indirect { at, .. } => at.as_ref(),
                loc => loc,
            };
            match loc {
                Loc::Reg(carrier) => self.receive(v, *carrier),
                Loc::Stack { offset, .. } => self.home_at(v, CALLER_ARGS + offset_of(*offset)),
                Loc::Split(parts) => {
                    let buffer = self.place(v).expect("placed by the prologue");
                    for part in parts {
                        let disp = buffer.disp + i64::try_from(part.offset).expect("records fit memory");
                        self.store_part(FP, disp, part.reg, part.bytes);
                    }
                }
                Loc::Copied { .. } | Loc::Returned | Loc::Indirect { .. } => {}
            }
        }
        // An entry that is also jumped to receives its parameters in their homes.
        if !self.cfg.preds[entry].is_empty() {
            self.spill_live();
            self.clear();
        }
        self.unlock();
    }

    /// A buffer in the frame for a record of `parts`.
    fn buffer(&mut self, parts: &[crate::arm64::abi::Part]) -> i32 {
        let size = parts.iter().map(|p| p.offset + p.bytes).max().unwrap_or(0);
        self.frame.reserve(size.next_multiple_of(Arm::WORD), Arm::WORD)
    }
}

/// The moves building the frame's `size` in `x16`: its upper 16 bits, then its lower.
fn frame_size(code: &mut vun_asm_arm64::Asm<vun_lir::Address>, size: u64) {
    assert!(size >> (2 * HALF) == 0, "frames fit 4 GiB");
    let (high, low) = (size >> HALF << HALF, size & 0xffff);
    code.emit(Op::Mov, Size::B64, &[SIZE.into(), Operand::Imm(high.cast_signed())]);
    code.emit(Op::Movk, Size::B64, &[SIZE.into(), Operand::Imm(low.cast_signed())]);
}
