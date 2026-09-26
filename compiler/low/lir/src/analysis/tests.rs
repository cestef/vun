use vun_index::Idx;
use vun_intern::Symbol;
use vun_origin::{OriginId, OriginTable};

use super::{Cfg, Liveness, Location, Loops, Move, Moves, Order, Range, StackUse, Unbounded};
use crate::func::{Conv, Linkage, Signature};
use crate::tests::{origin, sum};
use crate::{AbiType, Block, Builder, Function, Scalar, Target, Terminator};

fn b(i: usize) -> Block {
    Block::new(i)
}

fn analyse(func: &Function) -> (Loops, Order) {
    let loops = Loops::of(func, &Cfg::of(func));
    let order = Order::of(func, &loops);
    (loops, order)
}

/// A function of one `i8` parameter whose blocks end as `ends` gives, each block `b1` onward
/// made first.
fn shaped(origin: OriginId, blocks: usize, ends: &dyn Fn(crate::Value) -> Vec<Terminator>) -> Function {
    let signature =
        Signature { conv: Conv::Vun, params: vec![AbiType::Int(Scalar::I8, crate::Ext::Zero)], result: None };
    let mut builder = Builder::new(Linkage::Export(Symbol::intern("f")), signature, origin);
    let cond = builder.params(builder.current())[0];
    for _ in 1..blocks {
        builder.block(&[], origin);
    }
    for (i, term) in ends(cond).into_iter().enumerate() {
        builder.switch_to(b(i));
        builder.terminate(term);
    }
    builder.finish()
}

fn to(i: usize) -> Target {
    Target { block: b(i), args: Vec::new() }
}

fn branch(cond: crate::Value, then: usize, otherwise: usize) -> Terminator {
    Terminator::Branch { cond, then: to(then), otherwise: to(otherwise) }
}

#[test]
fn a_counting_loop_is_one_span_and_its_bound_lives_through_it() {
    let origins = OriginTable::new();
    let unit = sum(origin(&origins));
    let func = &unit.functions[crate::FuncId::new(0)];
    let (loops, order) = analyse(func);
    assert_eq!(order.blocks(), [b(0), b(1), b(2), b(3)]);
    assert!(loops.is_header(b(1)) && !loops.is_irreducible(b(1)));
    assert_eq!(loops.innermost(b(2)), Some(b(1)));
    assert_eq!(loops.innermost(b(3)), None);
    assert_eq!(order.span(b(1)), (1, 2));
    let live = Liveness::of(func, &loops, &order);
    let [n, _, _, total] = [0, 1, 2, 3].map(crate::Value::new);
    assert_eq!(live.range(n), Some(Range { first: 0, last: 2, through: true, uses: 1 }));
    assert_eq!(live.range(total), Some(Range { first: 1, last: 3, through: false, uses: 2 }));
}

#[test]
fn a_block_jumping_to_itself_heads_its_own_loop() {
    let origins = OriginTable::new();
    let func = shaped(origin(&origins), 3, &|c| {
        vec![Terminator::Jump(to(1)), branch(c, 1, 2), Terminator::Return(None)]
    });
    let (loops, order) = analyse(&func);
    assert!(loops.is_header(b(1)));
    assert_eq!(loops.innermost(b(1)), Some(b(1)));
    assert_eq!(order.span(b(1)), (1, 1));
    // The condition, used in the loop but defined before it, lives to the loop's end.
    let live = Liveness::of(&func, &loops, &order);
    assert_eq!(live.range(crate::Value::new(0)), Some(Range { first: 0, last: 1, through: true, uses: 1 }));
}

#[test]
fn a_loop_is_emitted_before_what_follows_it() {
    let origins = OriginTable::new();
    // b1 tests, exiting to b3 before its body b2, which jumps back.
    let func = shaped(origin(&origins), 4, &|c| {
        vec![Terminator::Jump(to(1)), branch(c, 3, 2), Terminator::Jump(to(1)), Terminator::Return(None)]
    });
    let (loops, order) = analyse(&func);
    assert_eq!(loops.rpo(), [b(0), b(1), b(3), b(2)]);
    assert_eq!(order.blocks(), [b(0), b(1), b(2), b(3)]);
    assert_eq!(order.next(b(2)), Some(b(3)));
}

#[test]
fn a_cycle_entered_twice_is_an_irreducible_loop() {
    let origins = OriginTable::new();
    // b0 enters the cycle of b1 and b2 at either; b2 may leave to b3.
    let func = shaped(origin(&origins), 4, &|c| {
        vec![branch(c, 1, 2), Terminator::Jump(to(2)), branch(c, 1, 3), Terminator::Return(None)]
    });
    let (loops, order) = analyse(&func);
    let header = loops.innermost(b(1)).expect("b1 is in a loop");
    assert_eq!(loops.innermost(b(2)), Some(header));
    assert!(loops.is_irreducible(header));
    assert_eq!(loops.innermost(b(3)), None);
    let (start, end) = order.span(header);
    assert_eq!(end - start, 1, "the cycle is one span");
}

#[test]
fn swapped_parameters_move_through_a_temporary() {
    let origins = OriginTable::new();
    let mut unit = sum(origin(&origins));
    let func = &mut unit.functions[crate::FuncId::new(0)];
    let [i, total] = [func.blocks[b(1)].params[0], func.blocks[b(1)].params[1]];
    let target = Target { block: b(1), args: vec![total, i] };
    let moves = Moves::of(func, &target);
    let (v, temp) = (Location::Value, Location::Temp(Scalar::I64));
    assert_eq!(
        moves.0,
        [Move { to: temp, from: v(i) }, Move { to: v(i), from: v(total) }, Move { to: v(total), from: temp },]
    );
    let same = Target { block: b(1), args: vec![i, total] };
    assert_eq!(Moves::of(func, &same), Moves::default());
}

/// A unit of functions that each call those `calls` lists for it, by index.
fn calling(origin: OriginId, calls: &[&[usize]]) -> crate::Unit {
    let mut unit = crate::Unit { pointer_bits: 64, ..crate::Unit::default() };
    for callees in calls {
        let signature = Signature { conv: Conv::Vun, params: Vec::new(), result: None };
        let mut builder = Builder::new(Linkage::Internal, signature, origin);
        for &callee in *callees {
            let call = crate::InstKind::Call {
                callee: crate::Callee::Func(crate::FuncId::new(callee)),
                args: Vec::new(),
            };
            builder.push(call, None, origin);
        }
        builder.terminate(Terminator::Return(None));
        unit.functions.push(builder.finish());
    }
    unit
}

#[test]
fn stack_use_is_a_frame_and_its_deepest_call() {
    let origin = origin(&OriginTable::new());
    let f = crate::FuncId::new;
    let unit = calling(origin, &[&[1, 2], &[2], &[]]);
    let uses = StackUse::of(&unit, &[(f(0), 16), (f(1), 32), (f(2), 48)]);
    assert_eq!([0, 1, 2].map(|i| uses.of_function(f(i))), [Ok(96), Ok(80), Ok(48)]);
    let unreported = StackUse::of(&unit, &[(f(0), 16), (f(2), 48)]);
    assert_eq!(unreported.of_function(f(0)), Err(Unbounded::Unreported(f(1))));
}

#[test]
fn recursion_leaves_stack_use_unbounded() {
    let origin = origin(&OriginTable::new());
    let f = crate::FuncId::new;
    let unit = calling(origin, &[&[1], &[0], &[]]);
    let uses = StackUse::of(&unit, &[(f(0), 16), (f(1), 16), (f(2), 16)]);
    assert_eq!(uses.of_function(f(0)), Err(Unbounded::Recursive(f(0))));
    assert_eq!(uses.of_function(f(2)), Ok(16));
}
