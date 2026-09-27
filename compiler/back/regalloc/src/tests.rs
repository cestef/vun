use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::{
    BinOp, Block, Builder, Callee, Cond, FuncId, Function, InstKind, Linkage, Scalar, Target, Terminator,
};
use vun_origin::{OriginId, OriginTable};
use vun_span::{SourceFiles, Span};

use crate::sample::{Random, Toy, signature};
use crate::{Allocation, PReg};

fn origin(origins: &OriginTable) -> OriginId {
    let file = SourceFiles::new().add("a.vun", "f := () {};");
    origins.source(Span::new(file, 0, 1))
}

/// Allocates `f`, well formed, on a machine of `regs` registers and checks the plan.
fn allocates(f: &Function, regs: u8, preserved: u8) -> Allocation {
    let mut unit = vun_lir::Unit { pointer_bits: 64, ..vun_lir::Unit::default() };
    unit.functions.push(f.clone());
    assert_eq!(unit.verify_structure(), Ok(()), "{unit}");
    let toy = Toy::new(f, regs, preserved);
    let plan = Allocation::of(f, &toy);
    if let Err(wrong) = plan.check(f, &toy) {
        panic!("{wrong} with {regs} registers, {preserved} preserved:\n{plan}");
    }
    plan
}

/// `sum(n)`: adds `0..n` in a loop whose counter and total are block parameters, calling itself
/// on each total when `calls`.
fn sum(o: OriginId, calls: bool) -> Function {
    let mut b = Builder::new(Linkage::Export(Symbol::intern("sum")), signature(1), o);
    let n = b.params(b.current())[0];
    let zero = b.constant(Scalar::I64, 0, o);
    let head = b.block(&[Scalar::I64, Scalar::I64], o);
    let (body, exit) = (b.block(&[], o), b.block(&[], o));
    b.terminate(Terminator::Jump(Target { block: head, args: vec![zero, zero] }));
    b.switch_to(head);
    let [i, total] = [b.params(head)[0], b.params(head)[1]];
    let more = b.compare(Cond::ULt, i, n, o);
    let to = |block| Target { block, args: Vec::new() };
    b.terminate(Terminator::Branch { cond: more, then: to(body), otherwise: to(exit) });
    b.switch_to(body);
    let one = b.constant(Scalar::I64, 1, o);
    let next = b.binary(BinOp::Add, i, one, o);
    let mut added = b.binary(BinOp::Add, total, i, o);
    if calls {
        let call = InstKind::Call { callee: Callee::Func(FuncId::new(0)), args: vec![added] };
        added = b.push(call, Some(Scalar::I64), o).unwrap();
    }
    b.terminate(Terminator::Jump(Target { block: head, args: vec![next, added] }));
    b.switch_to(exit);
    b.terminate(Terminator::Return(Some(total)));
    b.finish()
}

#[test]
fn a_loop_keeps_its_state_in_registers() {
    let origins = OriginTable::new();
    let f = sum(origin(&origins), false);
    let plan = allocates(&f, 6, 2);
    // Nothing is spilled: every piece of every value is in a register.
    let homes = f.values.iter().filter_map(|(v, _)| plan.pieces.get(v)).flatten();
    assert!(homes.into_iter().all(|p| matches!(p.at, crate::Loc::Reg(_))), "{:?}", plan.pieces);
}

#[test]
fn values_live_across_a_call_take_preserved_registers_or_their_homes() {
    let origins = OriginTable::new();
    let f = sum(origin(&origins), true);
    for (regs, preserved) in [(6, 2), (6, 0), (3, 1), (4, 0)] {
        allocates(&f, regs, preserved);
    }
}

#[test]
fn few_registers_spill_and_reload() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    let mut b = Builder::new(Linkage::Internal, signature(4), o);
    let params = b.params(b.current()).to_vec();
    // Every sum of two parameters, all live until they are added up at the end.
    let mut sums = Vec::new();
    for (i, &x) in params.iter().enumerate() {
        for &y in &params[i..] {
            sums.push(b.binary(BinOp::Mul, x, y, o));
        }
    }
    let total = sums.iter().skip(1).fold(sums[0], |acc, &s| b.binary(BinOp::Add, acc, s, o));
    b.terminate(Terminator::Return(Some(total)));
    let f = b.finish();
    for regs in 4..8 {
        allocates(&f, regs, 1);
    }
}

#[test]
fn the_checker_finds_a_value_moved_where_it_is_not() {
    let origins = OriginTable::new();
    let f = sum(origin(&origins), false);
    let toy = Toy::new(&f, 6, 2);
    let mut plan = Allocation::of(&f, &toy);
    // The loop's total said to be in another register than it is.
    let total = f.blocks[Block::new(1)].params[1];
    let piece = plan.pieces[total].first_mut().unwrap();
    let crate::Loc::Reg(r) = piece.at else { panic!("in a register") };
    piece.at = crate::Loc::Reg(PReg((r.0 + 1) % 6));
    assert!(plan.check(&f, &toy).is_err());
}

#[test]
fn random_functions_allocate_to_plans_the_checker_accepts() {
    let origins = OriginTable::new();
    let o = origin(&origins);
    for seed in 1..400_u64 {
        let mut random = Random(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for (regs, preserved) in [(3, 0), (4, 0), (4, 2), (5, 1), (8, 3)] {
            // Three arguments in the first three registers leave a fourth for the rest.
            let f = random.function(regs > 3, o);
            allocates(&f, regs, preserved);
        }
    }
}
