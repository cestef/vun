//! Thumb's inline assembly in native code: outputs are moved through `ldr` and `str`, or `vldr`
//! and `vstr`. A checked write calls the runtime between saves of every register a call may
//! overwrite, two more keeping the stack aligned, and of the flags, which `r4` holds across the
//! call; the address is built in `r0` from the operand's mode.

use vun_asm::{Blocks, Untaken};
use vun_asm_thumb::{
    Cc, Class, Emit, List, Mem, Op, Operand, Reg, Shift, ShiftKind, Size, Special, Thumb, V,
};
use vun_codegen::{Arch, Code, Cx};
use vun_inline::RegRef;
use vun_lir::{Address, AsmCheck};

use super::asm::AsmExt;
use super::{Instr, Thumb2};
use vun_native::inline::{Inline, Inst};

/// The registers kept around a checked write: those a call may overwrite, and `r4`, holding the
/// flags, with `r11` keeping the stack aligned to pairs of words.
const KEPT: List = List::Regs(
    1 << Reg::R0.number()
        | 1 << Reg::R1.number()
        | 1 << Reg::R2.number()
        | 1 << Reg::R3.number()
        | 1 << FLAGS.number()
        | 1 << Reg::R11.number()
        | 1 << Reg::R12.number()
        | 1 << Reg::Lr.number(),
);

/// The bytes `KEPT` takes on the stack.
const KEPT_BYTES: i64 = 32;
/// The bytes of a pair of words, which the stack stays aligned to.
const PAIR: i64 = 8;

/// Where a checked write's address is built, and the register holding the flags, which a call
/// preserves.
const ADDRESS: Reg = Reg::R0;
const FLAGS: Reg = Reg::R4;

impl Inline for Thumb2 {
    type Table = Thumb;

    fn emit(code: &mut Code<Self>, inst: &Inst, operands: &[Operand<Address>]) -> Result<(), Untaken> {
        let (op, form) = Op::entry(inst.op, inst.form);
        code.emit_by(op, form, Size::of(inst.size), operands)
    }

    fn check_write(
        code: &mut Code<Self>,
        at: &Operand<Address>,
        bytes: u32,
        floats: &[u8],
        check: &AsmCheck,
    ) {
        code.emit(Op::Push, Size::B32, &[Operand::List(KEPT)]);
        // Each floating-point register in a word of its own, the stack kept aligned to pairs.
        let spilled = PAIR * i64::try_from(floats.len().div_ceil(2)).expect("a few pairs");
        if spilled > 0 {
            code.offset(Reg::Sp, Reg::Sp, -spilled);
        }
        for (k, &v) in (0_i64..).zip(floats) {
            code.emit(Op::Vstr, Size::B32, &[V::ALL[usize::from(v)].into(), Mem::at(Reg::Sp, 4 * k).into()]);
        }
        code.emit(Op::Mrs, Size::B32, &[FLAGS.into(), Operand::Special(Special::Apsr)]);
        // An address off the stack pointer is further from it by what was saved.
        let saved = KEPT_BYTES + spilled;
        let off = |base: Reg| if base == Reg::Sp { saved } else { 0 };
        let Operand::Mem(mem) = *at else { unreachable!("a write is to memory") };
        match mem {
            Mem::Off { base, offset } | Mem::Pre { base, offset } => {
                code.offset(ADDRESS, base, offset + off(base));
            }
            Mem::Post { base, .. } => code.offset(ADDRESS, base, off(base)),
            // The base is built where the index is not.
            Mem::Index { base, index, shift } => {
                let built = if index == ADDRESS { Reg::R1 } else { ADDRESS };
                code.offset(built, base, off(base));
                let index = match shift {
                    0 => Operand::Reg(index),
                    amount => Operand::Shifted(index, Shift { kind: ShiftKind::Lsl, amount }),
                };
                code.emit(Op::Add, Size::B32, &[ADDRESS.into(), built.into(), index]);
            }
        }
        code.constant(Reg::R1, u64::from(bytes));
        code.constant(Reg::R2, u64::from(check.site));
        code.emit(Op::Bl, Size::B32, &[Operand::Sym(check.write)]);
        code.emit(Op::Msr, Size::B32, &[Operand::Special(Special::Apsr), FLAGS.into()]);
        for (k, &v) in (0_i64..).zip(floats) {
            code.emit(Op::Vldr, Size::B32, &[V::ALL[usize::from(v)].into(), Mem::at(Reg::Sp, 4 * k).into()]);
        }
        if spilled > 0 {
            code.offset(Reg::Sp, Reg::Sp, spilled);
        }
        code.emit(Op::Pop, Size::B32, &[Operand::List(KEPT)]);
    }

    fn stack(code: &mut Code<Self>, dst: Reg) {
        code.emit(Op::Mov, Size::B32, &[dst.into(), Reg::Sp.into()]);
    }

    /// The stack pointer is read into the framework's own register, which compares take; the
    /// arguments are moved so that none is overwritten before it is read.
    fn check_stack(code: &mut Code<Self>, before: Reg, check: &AsmCheck) {
        let kept = code.label();
        code.emit(Op::Mov, Size::B32, &[Thumb2::TEMP.into(), Reg::Sp.into()]);
        code.emit(Op::Cmp, Size::B32, &[Thumb2::TEMP.into(), before.into()]);
        code.emit(Op::Bcond, Size::B32, &[Cc::Eq.into(), kept.into()]);
        code.emit(Op::Mov, Size::B32, &[Reg::R1.into(), before.into()]);
        code.emit(Op::Mov, Size::B32, &[Reg::R2.into(), Thumb2::TEMP.into()]);
        code.constant(Reg::R0, u64::from(check.site));
        code.emit(Op::Bl, Size::B32, &[Operand::Sym(check.stack)]);
        code.bind(kept);
    }

    fn floats(cx: &Cx<'_, Self>) -> Vec<RegRef> {
        cx.state.float_args().iter().map(|v| RegRef { class: Class::Fp.id(), number: v.number() }).collect()
    }

    /// `mov` between general registers, `vmov` into a floating-point one: a word either way, as
    /// no value is wider.
    fn moved(code: &mut Code<Self>, dst: RegRef, src: Reg, _: Size) {
        let op = if Thumb::general(dst).is_some() { Op::Mov } else { Op::Vmov };
        code.emit(op, Size::B32, &[Thumb::register(dst), src.into()]);
    }

    fn load_at(code: &mut Code<Self>, r: RegRef, addr: Reg, size: Size) {
        let (op, size) = if Thumb::general(r).is_some() { Instr::Load.at(size) } else { (Op::Vldr, size) };
        code.emit(op, size, &[Thumb::register(r), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: RegRef, size: Size) {
        let (op, size) = if Thumb::general(r).is_some() { Instr::Store.at(size) } else { (Op::Vstr, size) };
        code.emit(op, size, &[Thumb::register(r), Mem::at(addr, 0).into()]);
    }
}
