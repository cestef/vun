//! Naked functions as GNU C writes them: a body of plain `__asm__` alone, whose blocks name
//! symbols as the object does, and what a block names kept.

use vun_backend::{Lack, Missing};
use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{Address, Builder, Conv, FuncId, Linkage, Scalar, Signature, Terminator, Unit};
use vun_origin::{OriginId, OriginTable};
use vun_target::Attributes;

use super::{NoLines, origin, returning, target};
use crate::CBackend;
use crate::ast::{Attribute, Item, Stmt};
use crate::emit::Emitter;

/// `start`, exported and naked, whose block calls the internal function after it.
fn calling(origin: OriginId) -> Unit {
    use vun_asm_x86_64::{Op, Size};
    use vun_inline::{Block, FormId, Inst, OpId, Operand, SizeId};
    let signature = Signature { conv: Conv::Naked, params: Vec::new(), result: None, variadic: None };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("start")), signature, origin);
    let index =
        |all: &[Op], op| u16::try_from(all.iter().position(|&o| o == op).expect("call")).expect("few");
    let size = u8::try_from(Size::ALL.iter().position(|&s| s == Size::B64).expect("64 bits")).expect("few");
    let inst = Inst {
        op: OpId(index(Op::ALL, Op::Call)),
        form: FormId(0),
        size: SizeId(size),
        operands: vec![Operand::Symbol(Address::Func(FuncId::new(1)))],
        origin,
    };
    let block =
        Block { arch: vun_items::lang::arch::x86_64, insts: vec![inst], returns: true, needs: Vec::new() };
    b.push(vun_lir::InstKind::Asm(Box::new(block), None), None, origin);
    b.terminate(Terminator::Unreachable);
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit.functions.push(returning(Linkage::Internal, Scalar::I32, origin));
    unit
}

#[test]
fn a_naked_function_is_plain_gnu_assembly_calling_by_symbol() {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let unit = calling(at);
    let emitted = |attributes, format: &str| {
        let mut target = target();
        target.c.compiler.attributes = attributes;
        target.object_format = Symbol::intern(format);
        Emitter::sources(&unit, &target, &NoLines, usize::MAX)
    };
    assert_eq!(emitted(Attributes::None, "elf").err(), Some(Lack { missing: Missing::Naked, at }));
    let symbol = Missing::AsmOperand(vun_asm::AsmOperand::Symbol);
    assert_eq!(emitted(Attributes::Gnu, "macho").err(), Some(Lack { missing: symbol, at }));
    let sources = emitted(Attributes::Gnu, "elf").unwrap();
    let items: Vec<&Item> = sources.iter().flat_map(|s| &s.file.items).collect();
    let declared: Vec<&[Attribute]> = items
        .iter()
        .filter_map(|item| match item {
            Item::Declare(_, attributes) => Some(attributes.as_slice()),
            _ => None,
        })
        .collect();
    assert_eq!(declared, [&[Attribute::Naked][..], &[Attribute::Used]]);
    let naked = items.iter().find_map(|item| match item {
        Item::Define(proto, body)
            if proto.params.is_empty() && body.iter().any(|s| matches!(s, Stmt::Asm(_))) =>
        {
            Some(body)
        }
        _ => None,
    });
    let asm = naked.expect("the naked function").iter().filter_map(|s| match s {
        Stmt::Asm(asm) => Some(asm),
        _ => None,
    });
    assert!(asm.clone().count() == 1 && asm.clone().all(|a| a.bindings.is_empty() && a.clobbers.is_empty()));
}

#[test]
fn a_naked_function_compiles_with_the_machine_s_c_compilers() {
    let origins = OriginTable::new();
    let mut gnu = target();
    gnu.c.compiler.attributes = Attributes::Gnu;
    let sources = CBackend::default().sources(&calling(origin(&origins)), &gnu, &NoLines).unwrap();
    let [source] = sources.as_slice() else { panic!("one source") };
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let dir = vun_fs::Scratch::new("c-naked");
    let (program, object) = (dir.path().join("program.c"), dir.path().join("program.o"));
    std::fs::write(&program, &source.bytes).unwrap();
    for cc in ["gcc", "clang"] {
        let compiled =
            std::process::Command::new(cc).args(["-O2", "-c"]).arg(&program).arg("-o").arg(&object).status();
        let Ok(compiled) = compiled else { continue };
        assert!(compiled.success(), "{cc} compiles:\n{}", String::from_utf8_lossy(&source.bytes));
    }
}
