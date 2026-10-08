use vun_diag::{DiagCtxt, DiagCx, DiagId, errors};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use super::Res;
use crate::lower::Source;
use crate::tree::ExprKind;
use crate::tree::Hir;

/// The diagnostics of resolving `text`, and how many names resolved inside the file.
fn check(text: &str) -> (Vec<DiagId>, usize) {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    assert_eq!(cx.diag.error_count(), 0, "{text:?} does not parse");
    let hir = Hir::lower(Source { cst: &cst, tokens: &tokens, text }, &mut cx);
    assert_eq!(hir.verify(cx.origins), Ok(()));
    let resolution = hir.resolve(&mut cx);
    let inside = hir
        .exprs
        .iter()
        .filter(|(id, e)| {
            matches!(e.kind, ExprKind::Name(_)) && matches!(resolution.names[*id], Some(Res::Binder(_)))
        })
        .count();
    (diag.sorted(&origins).into_iter().map(|d| d.id).collect(), inside)
}

#[test]
fn namespaces_are_order_independent_and_blocks_sequential() {
    assert_eq!(check("a := b; b := 1;"), (vec![], 1));
    assert_eq!(check("f := () { x := y; y := 1; };").1, 0);
}

#[test]
fn duplicates_and_shadowing() {
    let cases: &[(&str, &[DiagId])] = &[
        ("a := 1; a := 2;", &[errors::NAMES_DUPLICATE]),
        ("f := (a: u8) { a := 1; };", &[errors::NAMES_SHADOWED]),
        ("x := 1; f := () { x := 2; };", &[errors::NAMES_SHADOWED]),
        ("f := () { x := 1; { x := 2; } };", &[errors::NAMES_SHADOWED]),
        ("f := () { for i in xs { i := 1; } };", &[errors::NAMES_SHADOWED]),
        // Members of a nested namespace are reached through it.
        ("new := 1; Uart := struct { new := 2; };", &[]),
        // Functions do not capture, so an inner function's locals do not shadow outer ones.
        ("f := () { x := 1; g := () { x := 2; }; };", &[]),
        // Siblings do not shadow each other.
        ("f := () { { x := 1; } { x := 2; } };", &[]),
        ("f := () { match e { .a(x) => x, .b(x) => x } };", &[]),
        // Parameter names of a function type label positions and shadow nothing.
        ("symbol := 1; export: (symbol: u8) -> u8 = _;", &[]),
        ("F := (a: u8, a: u8) -> u8;", &[errors::NAMES_SHADOWED]),
        // Selected names are members of the namespace they are spread into.
        ("..lang.int.{ u8 }; u8 := 1;", &[errors::NAMES_DUPLICATE]),
    ];
    for &(text, expected) in cases {
        assert_eq!(check(text).0, expected, "{text:?}");
    }
}

#[test]
fn functions_see_compile_time_binders_and_captures() {
    // `T` is a compile-time parameter, visible in the nested function; `n` is captured.
    assert_eq!(check("P := ($T: type) -> type { struct { f := (x: T) -> T { x }; } };"), (vec![], 3));
    assert_eq!(check("f := () { n := 1; g := [n] () -> u8 { n }; h := () -> u8 { n }; };"), (vec![], 2));
}

#[test]
fn compile_time_locals_are_visible_in_their_value_and_nested_functions() {
    // `$R` names itself from a function inside its value; `$for` binders are compile-time.
    assert_eq!(check("F := () -> type { $R := struct { f := (r: &R) {}; }; R };"), (vec![], 2));
    assert_eq!(check("S := struct { $for r in rs { f := () -> u8 { r }; } };"), (vec![], 1));
}

#[test]
fn irrefutable_tests_are_reported() {
    assert_eq!(check("f := (p: (u8, u8)) { if (a, b) = p { } };").0, [errors::BINDING_IRREFUTABLE_TEST]);
    assert_eq!(check("f := (o: ?u8) { if .some(x) = o { } for .some(y) = o { } };").0, []);
    assert_eq!(check("f := (o: E) { if n := .a | .b = o { } };").0, []);
}

#[test]
fn each_dollar_escapes_one_stage() {
    let ok = "f := ($x: u8) { y := #{ #{ $$x } }; z := $x; w := #{ $x }; };";
    assert!(check(ok).0.is_empty());
    assert_eq!(check("f := ($x: u8) { y := $$x; };").0, [errors::META_ESCAPE]);
    assert_eq!(check("f := ($x: u8) { y := #{ $$$x }; };").0, [errors::META_ESCAPE]);
}
