//! The frame and the parameters: the prologue pushes the frame pointer and the link register,
//! reserves the frame, its size built in `r12` by two moves written when the function ends, and
//! saves `FAR`; parameters passed in memory are addresses recomputed where used, the others are
//! received where the convention passes them, and the plan moves them on from there.

use vun_asm_thumb::{Emit, List, Op, Operand, Reg, Size};
use vun_codegen::{Arch, Place};
use vun_dwarf::Step;
use vun_native::width::offset_of;

use super::Sel;
use crate::abi::{Carrier, Loc, Part, Ret};
use crate::asm::AsmExt;
use crate::{FAR, FP, LR, Thumb2};

/// Where the caller's stack arguments start, above the saved frame pointer and link register.
pub(super) const CALLER_ARGS: i32 = 8;
/// The register the frame size is built in.
const SIZE: Reg = Thumb2::CARRIER;

/// The frame pointer and the link register, as the prologue pushes them; the epilogue pops the
/// return address into the program counter.
const LINKAGE: List = List::Regs(1 << FP.number() | 1 << LR.number());
pub(super) const RETURN: List = List::Regs(1 << FP.number() | 1 << Reg::Pc.number());

impl Sel<'_, '_> {
    /// `push {r7, lr}; mov r7, sp; movw r12, -size; movt r12, -size; add sp, r12`, `FAR` saved,
    /// and where each parameter passed in memory is.
    pub(super) fn prologue(&mut self) {
        self.asm.emit(Op::Push, Size::B32, &[Operand::List(LINKAGE)]);
        self.unwound(Step::Pushed);
        self.asm.emit(Op::Mov, Size::B32, &[FP.into(), Reg::Sp.into()]);
        self.unwound(Step::Framed);
        self.state.frame_at = self.asm.here();
        self.asm.frame_size(SIZE, 0);
        self.asm.emit(Op::Add, Size::B32, &[Reg::Sp.into(), SIZE.into()]);
        let far = self.frame.reserve(Thumb2::WORD, Thumb2::WORD);
        Thumb2::store(&mut self.asm, far, FAR);
        self.state.far_at = far;
        let above = 2 * i64::try_from(Thumb2::WORD).expect("a word fits");
        self.unwound(Step::Saved { reg: Thumb2::dwarf(FAR), offset: i64::from(far) - above });
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
        let mut moves = vun_asm_thumb::Asm::listed();
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
                        self.asm.offset(Thumb2::TEMP, FP, at(rest.from));
                        self.asm.offset(Thumb2::CARRIER, FP, i64::from(CALLER_ARGS + offset_of(rest.offset)));
                        self.copy_bytes(Thumb2::TEMP, Thumb2::CARRIER, rest.bytes, FAR);
                    }
                }
                Loc::Reg(Carrier::Int(_)) | Loc::Copied { .. } | Loc::Returned | Loc::Indirect { .. } => {}
            }
        }
    }

    /// A buffer in the frame for a record of `size` bytes.
    fn buffer(&mut self, size: u64) -> i32 {
        self.frame.reserve(size.next_multiple_of(Thumb2::WORD), Thumb2::WORD)
    }
}
