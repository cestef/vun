//! The frame and the parameters: the prologue reserves the frame, its size written when the
//! function ends; parameters passed in memory are addresses recomputed where used, the others
//! are received where the convention passes them, and the plan moves them on from there.

use vun_asm_x86_64::{Emit, Inst, Mem, Op, Operand, Reg, Size};
use vun_codegen::{Arch, Place};
use vun_dwarf::Step;
use vun_native::width::offset_of;

use super::Sel;
use crate::X86;
use crate::abi::{ARGS, Carrier, Loc, Part, Ret};

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
                Loc::Split { parts, rest } => Place::frame(self.buffer(Part::extent(parts, rest.as_ref()))),
                Loc::Returned => {
                    let Ret::Split(parts) = &self.state.passing.ret else { continue };
                    let buffer = self.buffer(Part::extent(parts, None));
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

    /// Receives the parameters passed in SSE registers and on the stack, and the records whose
    /// parts are passed in registers; those in general registers are where the plan takes them.
    pub(super) fn entry(&mut self) {
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        let locs = self.state.passing.params.clone();
        for (&v, loc) in f.blocks[entry].params.iter().zip(&locs) {
            match loc {
                Loc::Reg(Carrier::Float(x)) => self.receive_float(v, *x),
                Loc::Stack { offset, .. } => self.home_at(v, CALLER_ARGS + offset_of(*offset)),
                Loc::Split { parts, rest } => {
                    let buffer = self.place(v).expect("placed by the prologue");
                    let at = |offset| Mem::at(Reg::Rbp, frame_offset(buffer) + offset_of(offset));
                    for part in parts {
                        self.store_part(at(part.offset), part.reg, X86::WORD);
                    }
                    // The rest the caller left on the stack joins the parts.
                    if let Some(rest) = rest {
                        let from = Mem::at(Reg::Rbp, CALLER_ARGS + offset_of(rest.offset));
                        self.copy_bytes(at(rest.from), from, rest.bytes, X86::TEMP);
                    }
                }
                Loc::Reg(Carrier::Int(_)) | Loc::Copied { .. } | Loc::Indirect { .. } | Loc::Returned => {}
            }
        }
        if self.state.passing.ret == Ret::Memory {
            let home = self.frame.reserve(X86::WORD, X86::WORD);
            self.asm.emit(Op::Mov, Size::B64, &[Mem::at(Reg::Rbp, home).into(), ARGS[0].into()]);
            self.state.hidden = Some(home);
        }
    }

    /// A buffer in the frame for a record of `size` bytes.
    fn buffer(&mut self, size: u64) -> i32 {
        self.frame.reserve(size.next_multiple_of(X86::WORD), X86::WORD)
    }
}

/// The offset from the frame pointer of a place in the frame.
pub(super) fn frame_offset(place: Place) -> i32 {
    i32::try_from(place.disp).expect("frames fit 2 GiB")
}
