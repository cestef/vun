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
    let int = AbiType::Scalar(Scalar::I64);
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
    assert!(text.contains("fn f0 export \"sum\": c(i64) -> i64 {"), "{text}");
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
