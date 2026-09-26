//! The frame and the parameters: the prologue reserves the frame, its size written when the
//! function ends; parameters passed in memory are addresses recomputed where used, the others
//! are received where the convention passes them once the callee-saved registers are chosen.

use vun_asm_x86_64::{Emit, Mem, Op, Operand, Reg, Size};
use vun_codegen::{Arch, Place};

use super::Sel;
use crate::x86_64::X86;
use crate::x86_64::abi::{ARGS, Carrier, Loc, Ret};

/// Where the caller's stack arguments start, above the saved frame pointer and return address.
pub(super) const CALLER_ARGS: i32 = 16;
/// The frame size's placeholder: the widest immediate, so that any size is written over it.
const FRAME_PLACEHOLDER: i64 = i32::MAX as i64;
/// The bytes of a field an immediate of 32 bits takes.
const IMM32: u64 = 4;

impl Sel<'_, '_> {
    /// `push rbp; mov rbp, rsp; sub rsp, frame`, and where each parameter passed in memory is.
    pub(super) fn prologue(&mut self) {
        self.asm.emit(Op::Push, Size::B64, &[Reg::Rbp.into()]);
        self.asm.emit(Op::Mov, Size::B64, &[Reg::Rbp.into(), Reg::Rsp.into()]);
        self.asm.emit(Op::Sub, Size::B64, &[Reg::Rsp.into(), Operand::Imm(FRAME_PLACEHOLDER)]);
        self.state.frame_at = self.asm.here() - IMM32;
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        for (i, &v) in f.blocks[entry].params.iter().enumerate() {
            let place = match &self.state.passing.params[i] {
                Loc::Copied { offset, .. } => Place::frame(CALLER_ARGS + offset_of(*offset)),
                Loc::Split(parts) => Place::frame(self.buffer(parts.len())),
                Loc::Returned => {
                    let Ret::Split(parts) = &self.state.passing.ret else { continue };
                    let buffer = self.buffer(parts.len());
                    self.state.gathered = Some(buffer);
                    Place::frame(buffer)
                }
                Loc::Reg(_) | Loc::Stack(_) => continue,
            };
            self.recompute_as(v, place);
        }
    }

    /// Receives the parameters passed in registers and on the stack.
    pub(super) fn entry(&mut self) {
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        let params = &f.blocks[entry].params;
        let locs = self.state.passing.params.clone();
        // Values in general registers are received first, so that one received from an SSE
        // register, which takes a general register, overwrites none still to be received.
        let sse = |loc: &Loc| matches!(loc, Loc::Reg(Carrier::Sse(_)));
        let ordered = params
            .iter()
            .zip(&locs)
            .filter(|(_, l)| !sse(l))
            .chain(params.iter().zip(&locs).filter(|(_, l)| sse(l)));
        for (&v, loc) in ordered {
            match loc {
                Loc::Reg(carrier) => self.receive(v, *carrier),
                Loc::Stack(offset) => self.home_at(v, CALLER_ARGS + offset_of(*offset)),
                Loc::Split(parts) => {
                    let buffer = self.place(v).expect("placed by the prologue");
                    for (k, &(carrier, _)) in parts.iter().enumerate() {
                        let at = Mem::at(Reg::Rbp, frame_offset(buffer) + offset_of(k as u64 * X86::WORD));
                        self.store_part(at, carrier, X86::WORD);
                    }
                }
                Loc::Copied { .. } | Loc::Returned => {}
            }
        }
        if self.state.passing.ret == Ret::Hidden {
            let home = self.frame.reserve(X86::WORD, X86::WORD);
            self.asm.emit(Op::Mov, Size::B64, &[Mem::at(Reg::Rbp, home).into(), ARGS[0].into()]);
            self.state.hidden = Some(home);
        }
        // An entry that is also jumped to receives its parameters in their homes.
        if !self.cfg.preds[entry].is_empty() {
            self.spill_live();
            self.clear();
        }
        self.unlock();
    }

    /// A buffer of `eightbytes` words in the frame.
    fn buffer(&mut self, eightbytes: usize) -> i32 {
        self.frame.reserve(eightbytes as u64 * X86::WORD, X86::WORD)
    }
}

/// An offset of the frame or of the arguments, which fit 2 GiB.
pub(super) fn offset_of(bytes: u64) -> i32 {
    i32::try_from(bytes).expect("frames and arguments fit 2 GiB")
}

/// The offset from the frame pointer of a place in the frame.
pub(super) fn frame_offset(place: Place) -> i32 {
    i32::try_from(place.disp).expect("frames fit 2 GiB")
}
