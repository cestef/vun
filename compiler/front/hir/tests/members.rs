//! What a namespace written as a struct can declare, read without evaluating it.

use vun_hir::tree::{ExprId, ExprKind, Hir, NsId};
use vun_intern::Symbol;

mod support;

/// The value of the first declaration of the file.
fn value(hir: &Hir) -> ExprId {
    let root = hir.root.expect("a root namespace");
    hir.decls[hir.member_decls(root)[0]].value.expect("a value")
}

/// The body of the struct the first declaration makes.
fn body(hir: &Hir) -> NsId {
    hir.made_struct(value(hir)).expect("a struct made")
}

fn lacks(text: &str, name: &str) -> bool {
    let hir = support::lowered(text);
    hir.lacks(body(&hir), Symbol::intern(name))
}

#[test]
fn a_struct_written_under_plain_names_lacks_the_others() {
    let text = "s := struct { a: u8, b := 1; $if true { c := 2; } };";
    assert!(!lacks(text, "a") && !lacks(text, "b") && !lacks(text, "c"));
    assert!(lacks(text, "d"));
}

#[test]
fn a_spread_or_a_computed_name_may_bring_any_name() {
    assert!(!lacks("s := struct { ..other.names; };", "d"));
    assert!(!lacks("s := struct { $(name) := 1; };", "d"));
}

#[test]
fn a_function_literal_ending_in_a_struct_makes_it() {
    let text = "f := ($r: type) -> type { struct { json := 1; } };";
    assert!(lacks(text, "write") && !lacks(text, "json"));
    let hir = support::lowered("f := ($r: type) -> type { 1 };");
    let value = value(&hir);
    assert!(matches!(hir.exprs[value].kind, ExprKind::Fn { .. }) && hir.made_struct(value).is_none());
}
