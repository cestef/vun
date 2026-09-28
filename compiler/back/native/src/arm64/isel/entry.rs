//! The frame and the parameters: the prologue saves the frame pointer and the link register and
//! reserves the frame, its size built in `x16` by two moves written when the function ends;
//! parameters passed in memory are addresses recomputed where used, the others are received
//! where the convention passes them, and the plan moves them on from there.

use crate::width::offset_of;
use vun_asm_arm64::{Emit, Mem, Op, Reg, Size};
use vun_codegen::{Arch, Place};
use vun_dwarf::Step;

use super::Sel;
use crate::arm64::abi::{Carrier, Loc, Part, Ret};
use crate::arm64::asm::AsmExt;
use crate::arm64::{Arm, FAR, FP, LR};

/// Where the caller's stack arguments start, above the saved frame pointer and link register.
pub(super) const CALLER_ARGS: i32 = 16;
/// The bytes the prologue pushes: the frame pointer and the link register.
const PAIR: i64 = 16;
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
        self.asm.frame_size(SIZE, 0);
        self.asm.emit(Op::Sub, Size::B64, &[Reg::Sp.into(), Reg::Sp.into(), SIZE.into()]);
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        for (i, &v) in f.blocks[entry].params.iter().enumerate() {
            let place = match self.state.passing.params[i].clone() {
                Loc::Copied { offset, .. } => Place::frame(CALLER_ARGS + offset_of(offset)),
                Loc::Split { parts, rest } => Place::frame(self.buffer(Part::extent(&parts, rest.as_ref()))),
                Loc::Returned => {
                    let Ret::Split(parts) = self.state.passing.ret.clone() else { continue };
                    let buffer = self.buffer(Part::extent(&parts, None));
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
        moves.frame_size(SIZE, frame);
        let at = self.state.frame_at;
        self.asm.replace(at, moves.finish().expect("no labels"));
    }

    /// Receives the parameters passed in floating-point registers and on the stack, and the
    /// records whose parts are passed in registers; those in general registers are where the
    /// plan takes them.
    pub(super) fn entry(&mut self) {
        let f = self.f;
        let Some(entry) = f.entry() else { return };
        let locs = self.state.passing.params.clone();
        for (&v, loc) in f.blocks[entry].params.iter().zip(&locs) {
            // A record copied by the caller is received as its address.
            let loc = match loc {
                Loc::Indirect { at, .. } => at.as_ref(),
                loc => loc,
            };
            match loc {
                Loc::Reg(Carrier::Float(x)) => self.receive_float(v, *x),
                Loc::Stack { offset, .. } => self.home_at(v, CALLER_ARGS + offset_of(*offset)),
                Loc::Split { parts, rest } => {
                    let buffer = self.place(v).expect("placed by the prologue");
                    let at = |offset: u64| buffer.disp + i64::try_from(offset).expect("records fit memory");
                    for part in parts {
                        self.store_part(FP, at(part.offset), part.reg, part.bytes);
                    }
                    // The rest the caller left on the stack joins the parts.
                    if let Some(rest) = rest {
                        self.asm.offset(Arm::TEMP, FP, at(rest.from));
                        self.asm.offset(Arm::CARRIER, FP, i64::from(CALLER_ARGS + offset_of(rest.offset)));
                        self.copy_bytes(Arm::TEMP, Arm::CARRIER, rest.bytes, FAR);
                    }
                }
                Loc::Reg(Carrier::Int(_)) | Loc::Copied { .. } | Loc::Returned | Loc::Indirect { .. } => {}
            }
        }
    }

    /// A buffer in the frame for a record of `size` bytes.
    fn buffer(&mut self, size: u64) -> i32 {
        self.frame.reserve(size.next_multiple_of(Arm::WORD), Arm::WORD)
    }
}
