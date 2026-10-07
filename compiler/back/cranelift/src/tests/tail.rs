//! Tail calls: functions that tail call each other are bodies in the tail convention behind
//! entries, where every one of their group compiles here.

use vun_index::Idx;
use vun_intern::Symbol;
use vun_lir::analysis::Reached;
use vun_lir::testing::origin;
use vun_lir::{
    AbiType, Access, Address, Builder, CallKind, Callee, Conv, FuncId, InstKind, Linkage, Scalar, Signature,
    Terminator, Unit,
};
use vun_origin::OriginTable;

use super::{NAMES, isa, signature as plain};
use crate::compile::Compilation;
use crate::tail::Tails;

/// `main` calls `f`, which tail calls `g`; `extra` adds a function of the unit, given the id of
/// `g`.
fn chained(f: Linkage, extra: impl FnOnce(FuncId, &mut Unit, vun_origin::OriginId)) -> Unit {
    let origins = OriginTable::new();
    let at = origin(&origins);
    let int = AbiType::Int(Scalar::I64, vun_lir::Ext::Sign);
    let signature = || Signature {
        conv: Conv::Vun,
        params: vec![int.clone()],
        result: Some(int.clone()),
        variadic: None,
        convention: None,
    };
    let mut unit = Unit { pointer_bits: 64, ..Unit::default() };
    let mut b = Builder::new(Linkage::Internal, signature(), at);
    let n = b.params(b.current())[0];
    b.terminate(Terminator::Return(Some(n)));
    let g = unit.functions.push(b.finish());
    let mut b = Builder::new(f, signature(), at);
    let n = b.params(b.current())[0];
    let call = InstKind::Call { callee: Callee::Func(g), args: vec![n], kind: CallKind::Tail };
    let result = b.push(call, Some(Scalar::I64), at);
    b.terminate(Terminator::Return(result));
    let f = unit.functions.push(b.finish());
    let mut b = Builder::new(Linkage::Export(Symbol::intern("main")), signature(), at);
    let n = b.params(b.current())[0];
    let call = InstKind::Call { callee: Callee::Func(f), args: vec![n], kind: CallKind::Plain };
    let result = b.push(call, Some(Scalar::I64), at);
    b.terminate(Terminator::Return(result));
    unit.functions.push(b.finish());
    extra(g, &mut unit, at);
    unit
}

#[test]
fn functions_that_tail_call_each_other_compile_as_bodies_in_the_tail_convention_behind_entries() {
    let unit = chained(Linkage::Internal, |_, _, _| {});
    for name in NAMES {
        let compiled =
            Compilation::of(&unit, &isa(name.0), None, false).unwrap_or_else(|_| panic!("{name:?}"));
        assert!(compiled.code.iter().all(|(_, c)| !c.code.bytes.is_empty()), "{name:?}");
    }
}

#[test]
fn a_function_others_can_call_has_an_entry_in_the_targets_convention_before_its_body() {
    // `f` is exported and `g` has its address taken: both are reached through their entries.
    let unit = chained(Linkage::Export(Symbol::intern("f")), |g, unit, at| {
        let mut b = Builder::new(Linkage::Export(Symbol::intern("h")), plain(Conv::Vun, Vec::new()), at);
        let target = b.push(InstKind::Addr(Address::Func(g)), Some(Scalar::Ptr), at);
        b.push(
            InstKind::Store { value: target.unwrap(), addr: target.unwrap(), access: Access::Plain },
            None,
            at,
        );
        b.terminate(Terminator::Return(None));
        unit.functions.push(b.finish());
    });
    for name in NAMES {
        let compiled =
            Compilation::of(&unit, &isa(name.0), None, false).unwrap_or_else(|_| panic!("{name:?}"));
        // The entry of `f` calls its body, which is in the same code past the entry.
        let (f, code) = compiled.code.iter().nth(1).expect("f");
        let call =
            code.code.relocs.iter().find(|r| r.target == Address::Func(f)).expect("the entry calls the body");
        assert!(call.addend > 0, "{name:?}");
    }
}

#[test]
fn a_group_of_functions_one_of_which_another_backend_compiles_is_compiled_by_none_here() {
    let unit = chained(Linkage::Internal, |_, _, _| {});
    let reached = Reached::of(&unit);
    let [g, f] = [0, 1].map(FuncId::new);
    let tails = Tails::of(&unit, &reached, &[]);
    assert!(tails.contains(f) && tails.contains(g));
    // `f` goes to the assisting backend, whose jump to the entry of `g` would grow the stack.
    let tails = Tails::of(&unit, &reached, &[f]);
    assert!(!tails.contains(f) && !tails.contains(g));
}
