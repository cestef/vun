//! Blocks of inline assembly written as GNU C, compiled by the machine's C compilers.

use vun_intern::Symbol;
use vun_lir::{AbiType, Builder, Conv, Ext, Linkage, Scalar, Signature, Terminator, Unit};
use vun_origin::{OriginId, OriginTable};
use vun_target::{CInt, CTypes, Convention, TargetSpec};

use super::{NoLines, origin, target};
use crate::CBackend;

/// `f(n) -> i64` exported as `f`: `lea {out r}, [{in n} + 3]` into a slot, which it returns.
fn lea(origin: OriginId) -> Unit {
    use vun_asm_x86_64::{Op, Size};
    use vun_inline::{Base, Block, Bound, Disp, FormId, Inst, Mem, OpId, Operand, SizeId};
    let word = AbiType::Int(Scalar::I64, Ext::Sign);
    let signature =
        Signature { conv: Conv::C, params: vec![word.clone()], result: Some(word), variadic: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let n = b.params(b.current())[0];
    let slot = b.slot(8, 8, origin);
    let addr = b.push(vun_lir::InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin).expect("an address");
    let out = Operand::Bound(Bound::Out { place: vun_lir::Output { addr, ty: Scalar::I64 }, reg: None });
    let base = Base::Bound(Bound::In { value: n, reg: None });
    let address = Operand::Mem(Mem { base: Some(base), index: None, disp: Disp::Bytes(3) });
    let index = |all: &[Op], op| u16::try_from(all.iter().position(|&o| o == op).expect("lea")).expect("few");
    let size = u8::try_from(Size::ALL.iter().position(|&s| s == Size::B64).expect("64 bits")).expect("few");
    let inst = Inst {
        op: OpId(index(Op::ALL, Op::Lea)),
        form: FormId(0),
        size: SizeId(size),
        operands: vec![out, address],
        origin,
    };
    let block = Block { arch: vun_items::lang::arch::x86_64, insts: vec![inst], returns: true };
    b.push(vun_lir::InstKind::Asm(Box::new(block), None), None, origin);
    let r = b.load(Scalar::I64, addr, vun_lir::Access::Plain, origin);
    b.terminate(Terminator::Return(Some(r)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

#[test]
fn a_block_of_inline_assembly_runs_as_gnu_c() {
    let origins = OriginTable::new();
    let sources = CBackend::default().sources(&lea(origin(&origins)), &target(), &NoLines).unwrap();
    let [source] = sources.as_slice() else { panic!("one source") };
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let dir = vun_fs::Scratch::new("c-asm");
    let (program, main) = (dir.path().join("program.c"), dir.path().join("main.c"));
    std::fs::write(&program, &source.bytes).unwrap();
    std::fs::write(&main, "long f(long);\nint main(void) { return (int)f(4); }\n").unwrap();
    let built = dir.path().join("program");
    for cc in ["gcc", "clang"] {
        let compiled =
            std::process::Command::new(cc).arg("-O2").arg(&program).arg(&main).arg("-o").arg(&built).status();
        let Ok(compiled) = compiled else { continue };
        assert!(compiled.success(), "{cc} compiles:\n{}", String::from_utf8_lossy(&source.bytes));
        let ran = std::process::Command::new(&built).status().unwrap();
        assert_eq!(ran.code(), Some(7), "{cc}");
    }
}

/// A 32-bit M-profile target, its code in Thumb, calling C by AAPCS.
fn cortex_m4() -> TargetSpec {
    let name = Symbol::intern;
    TargetSpec {
        arch: name("arm"),
        isa: name("thumb"),
        os: name("bare"),
        abi: name("eabihf"),
        convention: Convention::AapcsVfp,
        pointer_bits: 32,
        register_bits: 32,
        floats: vun_target::Floats::ALL,
        c: CTypes { long: CInt { bits: 32, align: 4 }, pointer_align: 4, ..target().c },
        ..target()
    }
}

/// `f(n) -> i32` exported as `f`: `mrs {out r}, primask` into a slot, then `cpsid i`, returning
/// `r`.
fn masked(origin: OriginId) -> Unit {
    use vun_asm_thumb::{Class, Mask, Op, Size, Special};
    use vun_inline::{Block, Bound, ClassId, FormId, Inst, OpId, Operand, RegRef};
    let word = AbiType::Int(Scalar::I32, Ext::Sign);
    let signature =
        Signature { conv: Conv::C, params: vec![word.clone()], result: Some(word), variadic: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let slot = b.slot(4, 4, origin);
    let addr = b.push(vun_lir::InstKind::SlotAddr(slot), Some(Scalar::Ptr), origin).expect("an address");
    let out = Operand::Bound(Bound::Out { place: vun_lir::Output { addr, ty: Scalar::I32 }, reg: None });
    let primask =
        Operand::Reg(RegRef { class: ClassId(Class::Special as u8), number: Special::Primask as u8 });
    let op =
        |op: Op| OpId(u16::try_from(Op::ALL.iter().position(|&o| o == op).expect("an op")).expect("few"));
    let inst = |o, operands| Inst { op: op(o), form: FormId(0), size: Size::B32.id(), operands, origin };
    let insts =
        vec![inst(Op::Mrs, vec![out, primask]), inst(Op::Cpsid, vec![Operand::Choice(Mask::I as u8)])];
    let block = Block { arch: vun_items::lang::arch::thumb, insts, returns: true };
    b.push(vun_lir::InstKind::Asm(Box::new(block), None), None, origin);
    let r = b.load(Scalar::I32, addr, vun_lir::Access::Plain, origin);
    b.terminate(Terminator::Return(Some(r)));
    let mut unit = Unit { pointer_bits: 32, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

#[test]
fn a_block_of_thumb_is_written_as_gnu_c_for_the_m_profile() {
    let origins = OriginTable::new();
    let sources = CBackend::default().sources(&masked(origin(&origins)), &cortex_m4(), &NoLines).unwrap();
    let [source] = sources.as_slice() else { panic!("one source") };
    let dir = vun_fs::Scratch::new("c-asm-thumb");
    let (program, object) = (dir.path().join("program.c"), dir.path().join("program.o"));
    std::fs::write(&program, &source.bytes).unwrap();
    let target = ["--target=thumbv7em-none-eabihf", "-mcpu=cortex-m4", "-mfloat-abi=hard", "-O2", "-c"];
    let compiled =
        std::process::Command::new("clang").args(target).arg(&program).arg("-o").arg(&object).status();
    let Ok(compiled) = compiled else { return };
    assert!(compiled.success(), "clang compiles:\n{}", String::from_utf8_lossy(&source.bytes));
}
