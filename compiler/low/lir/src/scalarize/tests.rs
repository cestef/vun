use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};

use crate::func::{Alias, Conv, Linkage, Signature};
use crate::tests::origin;
use crate::{
    AbiType, Access, BinOp, Builder, CastOp, Ext, FuncId, Function, Import, ImportId, ImportKind, InstKind,
    Scalar, Terminator, Unit, Value,
};

/// `f(x: i32) -> i32`: a pair `{ i32, i8 }` of 8 bytes, built field by field from `x`, copied
/// whole to a second slot and its fields read back and added; with `escapes`, the second
/// slot's address is also passed to an import.
fn pair(at: OriginId, escapes: bool) -> Unit {
    let int = AbiType::Int(Scalar::I32, Ext::Zero);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("f")), signature, at);
    let x = b.params(b.current())[0];
    let [built, copied] = [b.slot(8, 4, at), b.slot(8, 4, at)];
    let field = |b: &mut Builder, base: Value, offset: u128| {
        let offset = b.constant(Scalar::I64, offset, at);
        b.offset(base, offset, at)
    };
    let base = b.slot_addr(built, at);
    b.store(x, base, Access::Plain, at);
    let flag = b.cast(CastOp::Trunc, x, Scalar::I8, at);
    let second = field(&mut b, base, 4);
    b.store(flag, second, Access::Plain, at);
    let to = b.slot_addr(copied, at);
    b.push(InstKind::Copy { to, from: base, bytes: 8 }, None, at);
    let first = b.load(Scalar::I32, to, Access::Plain, at);
    let second = field(&mut b, to, 4);
    let flag = b.load(Scalar::I8, second, Access::Plain, at);
    let flag = b.cast(CastOp::ZExt, flag, Scalar::I32, at);
    let sum = b.binary(BinOp::Add, first, flag, at);
    if escapes {
        let callee = crate::Callee::Import(ImportId::new(0));
        b.push(InstKind::Call { callee, args: vec![to] }, None, at);
    }
    b.terminate(Terminator::Return(Some(sum)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    let signature = Signature { conv: Conv::C, params: vec![AbiType::Address(Alias::Any)], result: None };
    let kind = ImportKind::Func(signature);
    unit.imports.push(Import { symbol: Symbol::intern("keep"), kind, origin: at, stack_max: None });
    unit
}

fn memory(f: &Function) -> usize {
    let kinds = f.blocks.iter().flat_map(|(_, b)| &b.insts).map(|&i| &f.insts[i].kind);
    kinds
        .filter(|k| matches!(k, InstKind::Load { .. } | InstKind::Store { .. } | InstKind::Copy { .. }))
        .count()
}

#[test]
fn a_pair_built_copied_and_read_back_never_reaches_memory() {
    let origins = OriginTable::new();
    let mut unit = pair(origin(&origins), false);
    let f = &mut unit.functions[FuncId::new(0)];
    assert_eq!(f.scalarize(64), 2, "both slots");
    // Each slot is its two fields, then the padding in an `i8` and an `i16`, aligned.
    assert_eq!(f.promote(64), 8);
    f.sweep();
    assert_eq!(unit.verify(&origins), Ok(()));
    let f = &unit.functions[FuncId::new(0)];
    assert!(f.slots.is_empty());
    assert_eq!(memory(f), 0);
}

#[test]
fn a_slot_whose_address_is_passed_stays_whole_and_the_copy_to_it_is_one_per_tile() {
    let origins = OriginTable::new();
    let mut unit = pair(origin(&origins), true);
    let f = &mut unit.functions[FuncId::new(0)];
    assert_eq!(f.scalarize(64), 1, "the pair built, not the one passed");
    assert_eq!(unit.verify(&origins), Ok(()));
    let f = &unit.functions[FuncId::new(0)];
    let copies = f.insts.iter().filter(|(_, i)| matches!(i.kind, InstKind::Copy { .. })).count();
    assert_eq!(copies, 0, "the copy is a load and a store per tile of the pair built");
}
