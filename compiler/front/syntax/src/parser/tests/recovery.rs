//! Errors are reported once, where they are found, and the tree keeps its shape and its bounds.

use vun_diag::{DiagId, errors};

use super::{parse_text, parsed};
use crate::parser::MAX_NESTING;

#[test]
fn errors_are_reported_once_and_the_tree_verifies() {
    let cases: &[(&str, &[DiagId])] = &[
        ("x := 1\ny := 2;", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := ;", &[errors::SYNTAX_EXPECTED_EXPRESSION]),
        ("x := f(a, b;", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
        ("x := f(a b);", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := a < b < c;", &[errors::SYNTAX_CHAINED_OPERATOR]),
        ("x := a.;", &[errors::SYNTAX_EXPECTED_NAME]),
        ("f(x);", &[errors::SYNTAX_EXPECTED_TOKEN]),
        (") x := 1;", &[errors::SYNTAX_EXPECTED_DECLARATION]),
        ("x := { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := @;", &[errors::SYNTAX_UNKNOWN_CHARACTER]),
        ("x := match a { .b c, };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := struct { a b };", &[errors::SYNTAX_EXPECTED_TOKEN]),
        ("x := (", &[errors::SYNTAX_UNCLOSED_DELIMITER]),
        ("/// dangling", &[errors::SYNTAX_EXPECTED_DECLARATION]),
    ];
    for &(text, expected) in cases {
        assert_eq!(parse_text(text).1, expected, "{text:?}");
    }
}

#[test]
fn deep_nesting_is_bounded() {
    let depth = MAX_NESTING as usize * 2;
    let text = format!("x := {}1{};", "(".repeat(depth), ")".repeat(depth));
    assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
    let text = format!("x := {}1;", "-".repeat(depth));
    assert_eq!(parse_text(&text).1, [errors::SYNTAX_NESTING_TOO_DEEP]);
}

#[test]
fn long_chains_are_bounded() {
    let links = MAX_NESTING as usize * 64;
    for text in [
        format!("x := 1{};", " + 1".repeat(links)),
        format!("x := a{};", ".b".repeat(links)),
        format!("x := f{};", "()".repeat(links)),
        format!("x := if a {{}}{} else {{}};", " else if a {}".repeat(links)),
    ] {
        let (cst, _, ids) = parsed(&text);
        assert_eq!(ids, [errors::SYNTAX_NESTING_TOO_DEEP], "{}", &text[..40]);
        assert!(cst.depth() <= MAX_NESTING as usize + 8, "depth {}", cst.depth());
    }
}

#[test]
fn recovered_trees_fit_their_slots() {
    // Inputs the fuzzer found: junk in a list, a dangling doc before a failed entry, brackets
    // before a group that is neither a closure nor an array type, and a member named by `$` and
    // nothing, and an attribute whose recovery took the `;` ending its entry.
    for text in [
        "[)#[",
        "///\n'''",
        "[](",
        "[a, b](c)",
        ".$",
        ".$.",
        ".$[",
        "#[\u{5};#[",
        "[2]@[2",
        ".>",
        "((\u{3}]",
        "[",
    ] {
        let (_, ids) = parse_text(text);
        assert!(!ids.is_empty(), "{text:?} should report");
    }
}
