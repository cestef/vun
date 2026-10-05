//! arm64's inline assembly in native code: outputs are moved through `ldr` and `str`. A checked
//! write calls the runtime between saves of every register a call may overwrite and of the flags
//! (`nzcv`), the address built in `x16` from the operand's mode.

use vun_asm::Registered;
use vun_asm::Untaken;
use vun_asm::Via;
use vun_asm_arm64::{Arm64, Cc, Class, Emit, Mem, Op, Operand, Reg, Size, SysReg, V};
use vun_codegen::Arch;
use vun_codegen::Code;
use vun_codegen::Cx;
use vun_inline::RegRef;
use vun_lir::{Address, AsmCheck};

use super::Arm;
use super::asm::AsmExt;
use vun_native::inline::{Inline, Inst};

/// The registers kept around a checked write, in pairs: those a call may overwrite, `x18` left
/// to the platform, and the link register, with the zero register filling its pair.
const KEPT: [[Reg; 2]; 10] = [
    [Reg::X0, Reg::X1],
    [Reg::X2, Reg::X3],
    [Reg::X4, Reg::X5],
    [Reg::X6, Reg::X7],
    [Reg::X8, Reg::X9],
    [Reg::X10, Reg::X11],
    [Reg::X12, Reg::X13],
    [Reg::X14, Reg::X15],
    [Reg::X16, Reg::X17],
    [Reg::X30, Reg::Zr],
];

/// The bytes a pair of registers takes on the stack, which stays aligned to them.
const PAIR: i64 = 16;

/// Where a checked write's address is built, and the flags are moved through: kept registers.
const ADDRESS: Reg = Reg::X16;
const FLAGS: Reg = Reg::X17;

impl Inline for Arm {
    type Table = Arm64;

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
        for [a, b] in KEPT {
            code.emit(
                Op::Stp,
                Size::B64,
                &[a.into(), b.into(), Mem::Pre { base: Reg::Sp, offset: -PAIR }.into()],
            );
        }
        // Each floating-point register in a word of its own, the stack kept aligned to pairs.
        let spilled = PAIR * i64::try_from(floats.len().div_ceil(2)).expect("a few pairs");
        if spilled > 0 {
            code.emit(Op::Sub, Size::B64, &[Reg::Sp.into(), Reg::Sp.into(), spilled.into()]);
        }
        for (k, &v) in (0_i64..).zip(floats) {
            code.emit(Op::Str, Size::B64, &[V(v).into(), Mem::at(Reg::Sp, 8 * k).into()]);
        }
        // An address off the stack pointer is further from it by what was saved.
        let saved = PAIR * i64::try_from(KEPT.len()).expect("a few pairs") + spilled;
        let off = |base: Reg| if base == Reg::Sp { saved } else { 0 };
        let Operand::Mem(mem) = *at else { unreachable!("a write is to memory") };
        match mem {
            Mem::Off { base, offset } | Mem::Pre { base, offset } => {
                code.offset(ADDRESS, base, offset + off(base));
            }
            Mem::Post { base, .. } => code.offset(ADDRESS, base, off(base)),
            // The base is built where the index is not.
            Mem::Index { base, index, .. } => {
                let built = if index == ADDRESS { FLAGS } else { ADDRESS };
                code.offset(built, base, off(base));
                code.emit(Op::Add, Size::B64, &[ADDRESS.into(), built.into(), index.into()]);
            }
            Mem::Lo12 { base, target, .. } => {
                code.offset(ADDRESS, base, off(base));
                code.emit(
                    Op::Add,
                    Size::B64,
                    &[ADDRESS.into(), ADDRESS.into(), Operand::Lo12(target, Via::Direct)],
                );
            }
        }
        code.emit(Op::Mrs, Size::B64, &[FLAGS.into(), Operand::Sys(SysReg::Nzcv)]);
        code.emit(Op::Str, Size::B64, &[FLAGS.into(), Mem::Pre { base: Reg::Sp, offset: -PAIR }.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::X0.into(), ADDRESS.into()]);
        code.constant(Reg::X1, u64::from(bytes));
        code.constant(Reg::X2, u64::from(check.site));
        code.emit(Op::Bl, Size::B64, &[Operand::Sym(check.write)]);
        code.emit(Op::Ldr, Size::B64, &[FLAGS.into(), Mem::Post { base: Reg::Sp, offset: PAIR }.into()]);
        code.emit(Op::Msr, Size::B64, &[Operand::Sys(SysReg::Nzcv), FLAGS.into()]);
        for (k, &v) in (0_i64..).zip(floats) {
            code.emit(Op::Ldr, Size::B64, &[V(v).into(), Mem::at(Reg::Sp, 8 * k).into()]);
        }
        if spilled > 0 {
            code.emit(Op::Add, Size::B64, &[Reg::Sp.into(), Reg::Sp.into(), spilled.into()]);
        }
        for [a, b] in KEPT.into_iter().rev() {
            code.emit(
                Op::Ldp,
                Size::B64,
                &[a.into(), b.into(), Mem::Post { base: Reg::Sp, offset: PAIR }.into()],
            );
        }
    }

    fn stack(code: &mut Code<Self>, dst: Reg) {
        code.emit(Op::Mov, Size::B64, &[dst.into(), Reg::Sp.into()]);
    }

    /// The stack pointer is read into the framework's own register, which compares take; the
    /// arguments are moved so that none is overwritten before it is read.
    fn check_stack(code: &mut Code<Self>, before: Reg, check: &AsmCheck) {
        let kept = code.label();
        code.emit(Op::Mov, Size::B64, &[Arm::TEMP.into(), Reg::Sp.into()]);
        code.emit(Op::Cmp, Size::B64, &[Arm::TEMP.into(), before.into()]);
        code.emit(Op::Bcond, Size::B64, &[Cc::Eq.into(), kept.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::X1.into(), before.into()]);
        code.emit(Op::Mov, Size::B64, &[Reg::X2.into(), Arm::TEMP.into()]);
        code.constant(Reg::X0, u64::from(check.site));
        code.emit(Op::Bl, Size::B64, &[Operand::Sym(check.stack)]);
        code.bind(kept);
    }

    fn floats(cx: &Cx<'_, Self>) -> Vec<RegRef> {
        cx.state.float_args().iter().map(|v| RegRef { class: Class::Fp.id(), number: v.0 }).collect()
    }

    /// `mov` between general registers, `fmov` into a floating-point one.
    fn moved(code: &mut Code<Self>, dst: RegRef, src: Reg, size: Size) {
        let op = if Arm64::general(dst).is_some() { Op::Mov } else { Op::Fmov };
        code.emit(op, size, &[Arm64::register(dst), src.into()]);
    }

    fn load_at(code: &mut Code<Self>, r: RegRef, addr: Reg, size: Size) {
        code.emit(Op::Ldr, size, &[Arm64::register(r), Mem::at(addr, 0).into()]);
    }

    fn store_at(code: &mut Code<Self>, addr: Reg, r: RegRef, size: Size) {
        code.emit(Op::Str, size, &[Arm64::register(r), Mem::at(addr, 0).into()]);
    }
}
