//! What arm64 code needs beyond one instruction: constants, offsets, addresses and accesses in as
//! few instructions as the forms take, and the frame size a prologue builds.

use vun_asm::Via;
use vun_asm_arm64::{Asm, Emit, Mem, Op, Reg, Size};
use vun_codegen::Arch;
use vun_lir::Address;

use super::{Arm, FAR, Opnd};

/// The 16-bit halves of a 64-bit constant, and their bits.
const HALVES: u32 = 4;
const HALF: u32 = 16;

/// Sequences of instructions over an arm64 code buffer.
pub(crate) trait AsmExt {
    /// Writes the 64 bits `bits` into `dst`: in one move where one takes them, else a move of the
    /// first 16 bits differing from the rest, zeros or (where no 16 bits are zero and most are ones)
    /// ones, then a `movk` of each other differing 16 bits, never of zeros, which it cannot shift.
    ///
    /// # Panics
    /// Never: a constant no one move takes has 16 bits differing from the rest.
    fn constant(&mut self, dst: Reg, bits: u64);
    /// Writes `base + disp` into `dst`, the constant built in `dst` or, when that is the base, in
    /// `FAR`.
    fn offset(&mut self, dst: Reg, base: Reg, disp: i64);
    /// Writes the address of `target` into `dst`: its page and the offset within it, or for an
    /// import, its entry in the global offset table.
    fn address(&mut self, dst: Reg, target: Address);
    /// `op` of `size` between `reg` and memory `disp` bytes past `base`: at an offset where a form
    /// takes it, else at an index built in `scratch`.
    fn access(&mut self, op: Op, size: Size, reg: Opnd, base: Reg, disp: i64, scratch: Reg);
    /// Two moves building the frame's `size` in `dst`, its upper 16 bits then its lower, which the
    /// function's end rewrites once the size is known.
    fn frame_size(&mut self, dst: Reg, size: u64);
}

impl AsmExt for Asm<Address> {
    fn constant(&mut self, dst: Reg, bits: u64) {
        let v = bits.cast_signed();
        if Op::Mov.form(Size::B64, &[Opnd::Reg(dst), Opnd::Imm(v)]).is_some() {
            self.emit(Op::Mov, Size::B64, &[dst.into(), v.into()]);
            return;
        }
        let half = |k: u32| (bits >> (k * HALF)) & 0xffff;
        let ones = (0..HALVES).filter(|&k| half(k) == 0xffff).count();
        let zeros = (0..HALVES).filter(|&k| half(k) == 0).count();
        let fill = if ones > zeros && zeros == 0 { 0xffff } else { 0 };
        let differing: Vec<u32> = (0..HALVES).filter(|&k| half(k) != fill).collect();
        let (&first, rest) =
            differing.split_first().expect("a constant one move misses differs from its fill");
        let filled = (0..HALVES).filter(|&k| k != first).fold(0u64, |acc, k| acc | fill << (k * HALF));
        let start = filled | half(first) << (first * HALF);
        self.emit(Op::Mov, Size::B64, &[dst.into(), start.cast_signed().into()]);
        for &k in rest {
            self.emit(Op::Movk, Size::B64, &[dst.into(), (half(k) << (k * HALF)).cast_signed().into()]);
        }
    }

    fn offset(&mut self, dst: Reg, base: Reg, disp: i64) {
        let takes =
            |op: Op, n: i64| op.form(Size::B64, &[Opnd::Reg(dst), Opnd::Reg(base), Opnd::Imm(n)]).is_some();
        if disp == 0 {
            Arm::copy(self, dst, base);
        } else if takes(Op::Add, disp) {
            self.emit(Op::Add, Size::B64, &[dst.into(), base.into(), disp.into()]);
        } else if takes(Op::Sub, disp.wrapping_neg()) {
            self.emit(Op::Sub, Size::B64, &[dst.into(), base.into(), disp.wrapping_neg().into()]);
        } else {
            let scratch = if dst == base || dst == Reg::Sp { FAR } else { dst };
            self.constant(scratch, disp.cast_unsigned());
            self.emit(Op::Add, Size::B64, &[dst.into(), base.into(), scratch.into()]);
        }
    }

    fn address(&mut self, dst: Reg, target: Address) {
        match target {
            Address::Import(_) => {
                self.emit(Op::Adrp, Size::B64, &[dst.into(), Opnd::Page(target, Via::Got)]);
                let entry = Mem::Lo12 { base: dst, target, via: Via::Got };
                self.emit(Op::Ldr, Size::B64, &[dst.into(), entry.into()]);
            }
            Address::Func(_) | Address::Data(_) => {
                self.emit(Op::Adrp, Size::B64, &[dst.into(), Opnd::Page(target, Via::Direct)]);
                self.emit(Op::Add, Size::B64, &[dst.into(), dst.into(), Opnd::Lo12(target, Via::Direct)]);
            }
        }
    }

    fn access(&mut self, op: Op, size: Size, reg: Opnd, base: Reg, disp: i64, scratch: Reg) {
        let at = Opnd::Mem(Mem::at(base, disp));
        if op.reaches(size, reg, base, disp) {
            self.emit(op, size, &[reg, at]);
        } else {
            self.constant(scratch, disp.cast_unsigned());
            self.emit(op, size, &[reg, Mem::index(base, scratch).into()]);
        }
    }

    fn frame_size(&mut self, dst: Reg, size: u64) {
        assert!(size >> (2 * HALF) == 0, "frames fit 4 GiB");
        let (high, low) = (size >> HALF << HALF, size & 0xffff);
        self.emit(Op::Mov, Size::B64, &[dst.into(), Opnd::Imm(high.cast_signed())]);
        self.emit(Op::Movk, Size::B64, &[dst.into(), Opnd::Imm(low.cast_signed())]);
    }
}

/// What the forms of an instruction take.
pub(crate) trait OpExt {
    fn reaches(self, size: Size, reg: Opnd, base: Reg, disp: i64) -> bool;
}

impl OpExt for Op {
    fn reaches(self, size: Size, reg: Opnd, base: Reg, disp: i64) -> bool {
        self.form(size, &[reg, Opnd::Mem(Mem::at(base, disp))]).is_some()
    }
}
