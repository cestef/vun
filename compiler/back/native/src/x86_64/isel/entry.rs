//! The frame and the parameters: the prologue reserves the frame, its size written when the
//! function ends; parameters passed in memory are addresses recomputed where used, the others
//! are received where the convention passes them once the callee-saved registers are chosen.

use crate::width::offset_of;
use vun_asm_x86_64::{Emit, Inst, Mem, Op, Operand, Reg, Size};
use vun_codegen::{Arch, Place};
use vun_dwarf::Step;

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
        self.unwound(Step::Pushed);
        self.asm.emit(Op::Mov, Size::B64, &[Reg::Rbp.into(), Reg::Rsp.into()]);
        self.unwound(Step::Framed);
        self.state.frame_at = self.asm.here();
        self.asm.emit(Op::Sub, Size::B64, &[Reg::Rsp.into(), Operand::Imm(FRAME_PLACEHOLDER)]);
        self.state.frame_imm = self.asm.here() - IMM32;
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
                Loc::Reg(_) | Loc::Stack { .. } | Loc::Indirect { .. } => continue,
            };
            self.recompute_as(v, place);
        }
    }

    /// Writes the frame's size into the prologue's immediate, the last field of its `sub`.
    pub(super) fn frame(&mut self, frame: u64) {
        let (at, imm) = (self.state.frame_at, self.state.frame_imm);
        // The CPU sign-extends the immediate: a frame of 2 GiB or more would grow the stack down.
        let size = i32::try_from(frame).expect("frames fit 2 GiB");
        self.asm.overwrite(imm, &size.to_le_bytes());
        let sub = Inst::new(Op::Sub, Size::B64, &[Reg::Rsp.into(), Operand::Imm(i64::from(size))]);
        self.asm.relist(at, sub);
    }

    /// Receives the parameters passed in registers and on the stack.
    pub(super) fn entry(&mut self) {
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        let params = &f.blocks[entry].params;
        let locs = self.state.passing.params.clone();
        // Values in general registers are received first, so that one received from an SSE
        // register, which takes a general register, overwrites none still to be received.
        let sse = |loc: &Loc| matches!(loc, Loc::Reg(Carrier::Float(_)));
        let ordered = params
            .iter()
            .zip(&locs)
            .filter(|(_, l)| !sse(l))
            .chain(params.iter().zip(&locs).filter(|(_, l)| sse(l)));
        for (&v, loc) in ordered {
            match loc {
                Loc::Reg(carrier) => self.receive(v, *carrier),
                Loc::Stack { offset, .. } => self.home_at(v, CALLER_ARGS + offset_of(*offset)),
                Loc::Split(parts) => {
                    let buffer = self.place(v).expect("placed by the prologue");
                    for part in parts {
                        let at = Mem::at(Reg::Rbp, frame_offset(buffer) + offset_of(part.offset));
                        self.store_part(at, part.reg, X86::WORD);
                    }
                }
                Loc::Copied { .. } | Loc::Indirect { .. } | Loc::Returned => {}
            }
        }
        if self.state.passing.ret == Ret::Memory {
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

/// The offset from the frame pointer of a place in the frame.
pub(super) fn frame_offset(place: Place) -> i32 {
    i32::try_from(place.disp).expect("frames fit 2 GiB")
}
