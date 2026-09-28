use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use crate::func::{Conv, Linkage, Signature};
use crate::verify::{Entity, Reason};
use crate::{AbiType, BinOp, Builder, Cond, FuncId, InstKind, Scalar, Target, Terminator, Unit};

pub(crate) fn origin(origins: &OriginTable) -> OriginId {
    let file = SourceFiles::new().add("a.vun", "sum := (n: u64) -> u64 { ... };");
    origins.source(Span::new(file, 0, 3))
}

/// `sum(n)`: adds `0..n` in a loop whose counter and total are block parameters.
pub(crate) fn sum(origin: OriginId) -> Unit {
    let int = AbiType::Int(Scalar::I64, crate::Ext::Zero);
    let signature = Signature { conv: Conv::C, params: vec![int.clone()], result: Some(int) };
    let mut b = Builder::new(Linkage::Export(Symbol::intern("sum")), signature, origin);
    let n = b.params(b.current())[0];
    let zero = b.constant(Scalar::I64, 0, origin);
    let head = b.block(&[Scalar::I64, Scalar::I64], origin);
    let (body, exit) = (b.block(&[], origin), b.block(&[], origin));
    b.terminate(Terminator::Jump(Target { block: head, args: vec![zero, zero] }));
    b.switch_to(head);
    let [i, total] = [b.params(head)[0], b.params(head)[1]];
    let more = b.compare(Cond::ULt, i, n, origin);
    let to = |block| Target { block, args: Vec::new() };
    b.terminate(Terminator::Branch { cond: more, then: to(body), otherwise: to(exit) });
    b.switch_to(body);
    let one = b.constant(Scalar::I64, 1, origin);
    let (next, added) = (b.binary(BinOp::Add, i, one, origin), b.binary(BinOp::Add, total, i, origin));
    b.terminate(Terminator::Jump(Target { block: head, args: vec![next, added] }));
    b.switch_to(exit);
    b.terminate(Terminator::Return(Some(total)));
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    unit.functions.push(b.finish());
    unit
}

#[test]
fn the_verifier_accepts_a_loop_and_prints_it() {
    let origins = OriginTable::new();
    let unit = sum(origin(&origins));
    assert_eq!(unit.verify(&origins), Ok(()));
    let text = unit.to_string();
    assert!(text.contains("fn f0 export \"sum\": c(i64 zext) -> i64 zext {"), "{text}");
    assert!(text.contains("b1(v2: i64, v3: i64):"), "{text}");
    assert!(text.contains("  br v4, b2, b3"), "{text}");
}

#[test]
fn the_verifier_rejects_what_breaks_the_invariants() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let f = FuncId::new(0);
    let reason = |edit: &dyn Fn(&mut Unit)| {
        let mut unit = sum(o);
        edit(&mut unit);
        unit.verify(&origins).map_err(|m| (m.at, m.reason))
    };
    // The total read in the entry, where no path has defined it yet.
    let early = reason(&|u| {
        let func = &mut u.functions[f];
        let total = func.blocks[Idx::new(1)].params[1];
        func.blocks[Idx::new(0)].term = Terminator::Return(Some(total));
    });
    assert!(matches!(early, Err((_, Reason::NotDominated(_)))), "{early:?}");
    // The loop jumping back with one value for two parameters.
    let short = reason(&|u| {
        let func = &mut u.functions[f];
        let Terminator::Jump(target) = &mut func.blocks[Idx::new(2)].term else { unreachable!() };
        target.args.pop();
    });
    assert_eq!(short, Err((Entity::Block(f, Idx::new(2)), Reason::Arguments(Idx::new(1)))));
    // An addition of an integer and a float.
    let mixed = reason(&|u| {
        let func = &mut u.functions[f];
        let one = func
            .insts
            .iter()
            .find(|(_, d)| matches!(d.kind, InstKind::Const { bits: 1, .. }))
            .map(|(i, _)| i);
        func.insts[one.unwrap()].kind = InstKind::Const { ty: Scalar::F64, bits: 1 };
        let value = func.insts[one.unwrap()].result.unwrap();
        func.values[value].ty = Scalar::F64;
    });
    assert!(
        matches!(mixed, Err((_, Reason::Type { expected: Scalar::I64, found: Scalar::F64 }))),
        "{mixed:?}"
    );
    // The loop's test jumping to its head on both edges, with different totals.
    let edges = reason(&|u| {
        let func = &mut u.functions[f];
        let [i, total] = [func.blocks[Idx::new(1)].params[0], func.blocks[Idx::new(1)].params[1]];
        let Terminator::Branch { then, otherwise, .. } = &mut func.blocks[Idx::new(1)].term else {
            unreachable!()
        };
        *then = Target { block: Idx::new(1), args: vec![i, total] };
        *otherwise = Target { block: Idx::new(1), args: vec![total, i] };
    });
    assert_eq!(edges, Err((Entity::Block(f, Idx::new(1)), Reason::Edge(Idx::new(1)))));
    let elsewhere = OriginTable::new();
    assert_eq!(sum(o).verify(&elsewhere).map_err(|m| m.reason), Err(Reason::NoOrigin));
}

#[test]
fn the_verifier_rejects_an_offset_leaving_its_object() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    // Offsets of `steps` in turn from the address of an 8-byte slot.
    let verify = |steps: &[u64]| {
        let signature = Signature { conv: Conv::Vun, params: Vec::new(), result: None };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        let slot = b.slot(8, 8, o);
        let mut at = b.slot_addr(slot, o);
        for &step in steps {
            let step = b.constant(Scalar::I64, u128::from(step), o);
            at = b.offset(at, step, o);
        }
        b.terminate(Terminator::Return(None));
        let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
        unit.functions.push(b.finish());
        unit.verify(&origins).map_err(|m| m.reason)
    };
    assert_eq!(verify(&[4, 4]), Ok(()));
    assert_eq!(verify(&[4, 5]), Err(Reason::OutOfBounds { size: 8, at: 9 }));
    assert_eq!(verify(&[u64::MAX]), Err(Reason::OutOfBounds { size: 8, at: -1 }));
}

#[test]
fn the_verifier_rejects_a_malformed_shape() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let f = FuncId::new(0);
    let reason = |edit: &dyn Fn(&mut Unit)| {
        let mut unit = sum(o);
        edit(&mut unit);
        unit.verify(&origins).map_err(|m| (m.at, m.reason))
    };
    // The loop jumping back to the entry.
    let entered = reason(&|u| {
        let func = &mut u.functions[f];
        func.blocks[Idx::new(2)].term = Terminator::Jump(Target { block: Idx::new(0), args: Vec::new() });
    });
    assert_eq!(entered, Err((Entity::Block(f, Idx::new(2)), Reason::EntryTargeted)));
    // A switch on the counter with two cases of 1.
    let cases = reason(&|u| {
        let func = &mut u.functions[f];
        let i = func.blocks[Idx::new(1)].params[0];
        let to = |block| Target { block: Idx::new(block), args: Vec::new() };
        let cases = vec![(1, to(2)), (1, to(3))];
        func.blocks[Idx::new(1)].term = Terminator::Switch { value: i, cases, default: to(3) };
    });
    assert_eq!(cases, Err((Entity::Block(f, Idx::new(1)), Reason::DuplicateCase(1))));
    // A value claiming the definition of another.
    let claimed = reason(&|u| {
        let func = &mut u.functions[f];
        let [i, total] = [func.blocks[Idx::new(1)].params[0], func.blocks[Idx::new(1)].params[1]];
        func.values[total].def = func.values[i].def;
    });
    assert!(matches!(claimed, Err((_, Reason::Definition(_)))), "{claimed:?}");
    // A slot aligned to 3 bytes.
    let aligned = reason(&|u| {
        let func = &mut u.functions[f];
        func.slots.push(crate::func::SlotData { size: 3, align: 3, origin: o });
    });
    assert!(matches!(aligned, Err((Entity::Slot(..), Reason::Alignment(3)))), "{aligned:?}");
}

#[test]
fn an_import_prints_the_library_it_comes_from() {
    let origins = OriginTable::new();
    let mut unit = Unit::default();
    let library = Some(Symbol::intern("wasi_snapshot_preview1"));
    let kind = crate::ImportKind::Data;
    unit.imports.push(crate::Import {
        symbol: Symbol::intern("fd_write"),
        library,
        kind,
        origin: origin(&origins),
        stack_max: None,
    });
    assert!(unit.to_string().contains(r#"import i0 "fd_write" from "wasi_snapshot_preview1""#), "{unit}");
}

#[test]
fn the_verifier_checks_what_an_assembly_block_binds_and_where_it_ends() {
    use vun_inline::{Block, Bound, FormId, Inst, OpId, Operand, SizeId};
    let origins = OriginTable::new();
    let o = origin(&origins);
    // A block writing an 8-byte slot, or the constant 8 as if it were an address.
    let verify = |bind_slot: bool, returns: bool, term: Terminator| {
        let signature = Signature { conv: Conv::Vun, params: Vec::new(), result: None };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        let slot = b.slot(8, 8, o);
        let place = if bind_slot { b.slot_addr(slot, o) } else { b.constant(Scalar::I64, 8, o) };
        let place = crate::Output { addr: place, ty: Scalar::I64 };
        let operands = vec![Operand::Bound(Bound::Out { place, reg: None })];
        let inst = Inst { op: OpId(0), form: FormId(0), size: SizeId(3), operands, origin: o };
        let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
        b.push(InstKind::Asm(Box::new(Block { arch, insts: vec![inst], returns }), None), None, o);
        b.terminate(term);
        let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
        unit.functions.push(b.finish());
        unit.verify(&origins).map_err(|m| m.reason)
    };
    assert_eq!(verify(true, true, Terminator::Return(None)), Ok(()));
    assert_eq!(verify(true, false, Terminator::Unreachable), Ok(()));
    assert_eq!(verify(true, false, Terminator::Return(None)), Err(Reason::PastNever));
    let found = Scalar::I64;
    assert_eq!(
        verify(false, true, Terminator::Return(None)),
        Err(Reason::Type { expected: Scalar::Ptr, found })
    );
}

#[test]
fn a_naked_function_holds_nothing_but_blocks_without_bindings() {
    use vun_inline::{Block, FormId, Inst, OpId, SizeId};
    let origins = OriginTable::new();
    let o = origin(&origins);
    let arch = vun_items::lang::arch_x86_64::arith.parent().expect("a namespace");
    // A block naming nothing, then `slot` a slot, `constant` a constant, and `term`.
    let verify = |slot: bool, constant: bool, term: Terminator| {
        let signature = Signature { conv: Conv::Naked, params: Vec::new(), result: None };
        let mut b = Builder::new(Linkage::Internal, signature, o);
        if slot {
            b.slot(8, 8, o);
        }
        if constant {
            b.constant(Scalar::I64, 8, o);
        }
        let inst = Inst { op: OpId(0), form: FormId(0), size: SizeId(3), operands: Vec::new(), origin: o };
        b.push(InstKind::Asm(Box::new(Block { arch, insts: vec![inst], returns: false }), None), None, o);
        b.terminate(term);
        let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
        unit.functions.push(b.finish());
        unit.verify(&origins).map_err(|m| m.reason)
    };
    assert_eq!(verify(false, false, Terminator::Unreachable), Ok(()));
    assert_eq!(verify(true, false, Terminator::Unreachable), Err(Reason::Naked));
    assert_eq!(verify(false, true, Terminator::Unreachable), Err(Reason::Naked));
}

#[test]
fn constants_leave_the_loops_around_them_for_the_block_before() {
    let origins = OriginTable::new();
    let mut unit = sum(origin(&origins));
    let f = &mut unit.functions[FuncId::new(0)];
    let body = Idx::new(2);
    let one = f.blocks[body].insts[0];
    assert!(matches!(f.insts[one].kind, InstKind::Const { .. }));
    assert_eq!(f.hoist(), 1);
    assert!(f.blocks[Idx::new(0)].insts.contains(&one));
    assert!(!f.blocks[body].insts.contains(&one));
    assert_eq!(f.hoist(), 0);
    assert_eq!(unit.verify(&origins), Ok(()));
}
