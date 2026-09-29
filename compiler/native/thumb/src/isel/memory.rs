//! Memory: copies, through a register or in a loop; loads and stores are snippets, whose
//! addresses fold a slot's and constant offsets where a form reaches them, ordered ones between
//! barriers.

use vun_abi::{Access, Thread};
use vun_asm::RelocKind;
use vun_asm_thumb::{Cc, Coprocessor, Cr, Emit, Mem, Op, Operand, Reg, Size};
use vun_backend::{Lack, Missing};
use vun_codegen::Arch;
use vun_lir::{Address, DataId, Value};

use super::Sel;
use crate::Thumb2;
use crate::asm::AsmExt;
use vun_native::select::Carries;
use vun_native::width::offset_of;

/// Copies up to this many bytes move through a register, larger ones in a loop.
pub(super) const UNROLLED: u64 = 64;
/// The bytes a loop's step copies: a word.
const STEP: u64 = Thumb2::WORD;
/// The register holding the thread pointer, which the kernel sets and the program only reads
/// (`TPIDRURO`): the system control coprocessor's `c13, c0`, at `opc1` 0 and `opc2` 3.
const THREAD_POINTER: (Coprocessor, i64, Cr, Cr, i64) = (Coprocessor::P15, 0, Cr::C13, Cr::C0, 3);
/// The bytes of the word holding a datum's offset from the thread pointer.
const OFFSET_BYTES: u32 = 4;

impl Sel<'_, '_> {
    /// The address of the calling thread's storage of `data`, in the register the convention
    /// names (`r0`): the thread pointer, read into `r1`, plus the datum's offset from it, a word
    /// the linker writes, which the code loads from beside itself and branches around. The code
    /// of a program built for Thumb reaches what the linker places at its address, which only an
    /// executable knows, and so the offset.
    pub(super) fn thread_addr(&mut self, data: DataId) -> Result<(), Lack> {
        let Some(Thread { access: Access::Offset, address, .. }) = self.state.convention.thread else {
            return Err(self.lack(Missing::PerThread));
        };
        let pointer = Reg::R1;
        let (word, past) = (self.asm.label(), self.asm.label());
        self.asm.emit(Op::Ldr, Size::B32, &[address.into(), word.into()]);
        self.asm.emit(Op::BAlways, Size::B32, &[past.into()]);
        self.asm.word(word, OFFSET_BYTES, Address::Data(data), RelocKind::ThreadOffset32, 0);
        self.asm.bind(past);
        let (coprocessor, opc1, n, m, opc2) = THREAD_POINTER;
        self.asm.emit(
            Op::Mrc,
            Size::B32,
            &[
                Operand::Coprocessor(coprocessor),
                opc1.into(),
                pointer.into(),
                Operand::Cr(n),
                Operand::Cr(m),
                opc2.into(),
            ],
        );
        self.asm.emit(Op::Add, Size::B32, &[address.into(), address.into(), pointer.into()]);
        Ok(())
    }

    /// Copies `bytes` bytes, through a register or, past `UNROLLED`, in a loop of words.
    pub(super) fn copy(&mut self, to: Value, from: Value, bytes: u64) {
        if bytes <= UNROLLED {
            let (to, from) = (self.reg(to), self.reg(from));
            Thumb2::copy_bytes(self.0, to, from, bytes, Thumb2::CARRIER);
            return;
        }
        let (dst, src, left) = (self.scratch(), self.scratch(), self.scratch());
        self.reg_to(to, dst);
        self.reg_to(from, src);
        self.asm.constant(left, bytes / STEP);
        let again = self.asm.label();
        self.asm.bind(again);
        let step = i64::from(offset_of(STEP));
        let (read, written) = (Mem::Post { base: src, offset: step }, Mem::Post { base: dst, offset: step });
        self.asm.emit(Op::Ldr, Size::B32, &[Thumb2::CARRIER.into(), read.into()]);
        self.asm.emit(Op::Str, Size::B32, &[Thumb2::CARRIER.into(), written.into()]);
        self.asm.emit(Op::Subs, Size::B32, &[left.into(), left.into(), Operand::Imm(1)]);
        self.asm.emit(Op::Bcond, Size::B32, &[Cc::Ne.into(), again.into()]);
        Thumb2::copy_bytes(self.0, dst, src, bytes % STEP, Thumb2::CARRIER);
    }
}
