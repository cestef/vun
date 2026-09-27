//! Reductions under interests that name what they keep.

use crate::{Reduced, Uninteresting};

/// A file of several declarations, one of which holds the interesting call.
const FILE: &str = "\
helper := (a: u8, b: u8) -> u8 { a + b };

f := (x: u8) -> u8 {
    y := helper(x, 2);
    z := boom(y + 1);
    z * 3
};

g := 1;
";

#[test]
fn a_file_is_cut_down_to_the_least_that_parses_as_well_and_stays_interesting() {
    let mut interest = |text: &str| text.contains("boom(");
    let reduced = Reduced::of(FILE, &mut interest).unwrap();
    assert_eq!(reduced.text, "f := boom();\n");
    assert_eq!(reduced.stats.before, FILE.len());
    assert!(reduced.stats.kept > 0 && reduced.stats.tests > reduced.stats.kept);
}

#[test]
fn a_file_that_does_not_parse_may_lose_what_it_did_not_parse() {
    let broken = "f := (x: u8 { boom(x) ; } ;\ng := 1;\n";
    let mut interest = |text: &str| text.contains("boom");
    let reduced = Reduced::of(broken, &mut interest).unwrap();
    assert_eq!(reduced.text, "boom");
}

#[test]
fn an_uninteresting_original_is_refused() {
    let mut never = |_: &str| false;
    assert_eq!(Reduced::of(FILE, &mut never).unwrap_err(), Uninteresting);
}

#[test]
fn no_text_is_tested_twice() {
    let mut seen = std::collections::HashSet::new();
    let mut interest = |text: &str| {
        assert!(seen.insert(text.to_owned()), "tested twice: {text:?}");
        text.contains("boom")
    };
    Reduced::of(FILE, &mut interest).unwrap();
}
