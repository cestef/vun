use std::collections::BTreeMap;

use vun_fs::Scratch;

use super::Counts;
use super::count::Source;
use super::drift::Drift;

const TWO: &str = "// outside\nfn a() {\n    // one\n    let x = \"\n// in a string\n\";\n    /// not counted\n    //~ annotation\n    // SAFETY: held\n    let y = 1; // trailing\n    // two\n}\n\
struct S;\nimpl S {\n    fn m(&self) {\n        // three\n    }\n}\ntrait T {\n    fn d() {\n        // four\n    }\n    fn n();\n}\n";

fn counts(groups: &[(&str, u32)]) -> Counts {
    Counts(groups.iter().map(|(g, n)| ((*g).to_owned(), *n)).collect::<BTreeMap<_, _>>())
}

#[test]
fn only_plain_comment_lines_inside_function_bodies_count() {
    assert_eq!(Source::parse(TWO.to_owned()).unwrap().comments(), 4);
}

#[test]
fn a_file_that_is_not_rust_is_refused() {
    assert!(Source::parse("fn (".to_owned()).is_err());
}

#[test]
fn counts_are_grouped_and_generated_sources_are_skipped() {
    let root = Scratch::new("comments");
    root.write("compiler/a/x/src/lib.rs", TWO);
    root.write("compiler/a/y/src/lib.rs", TWO);
    root.write("compiler/b/x/src/lib.rs", TWO);
    root.write("compiler/b/x/src/generated/g.rs", TWO);
    root.write("compiler/base/diag/src/errors/e.rs", TWO);
    root.write("compiler/lang/items/src/i.rs", TWO);
    assert_eq!(Counts::of(root.path()).unwrap(), counts(&[("a", 8), ("b", 4)]));
}

#[test]
fn a_count_above_or_below_its_baseline_drifts() {
    let held = counts(&[("a", 4), ("b", 4), ("c", 1)]);
    let drifts = counts(&[("a", 5), ("b", 4), ("d", 2)]).drift(&held);
    let drift = |group: &str, counted, allowed| Drift { group: group.to_owned(), counted, allowed };
    assert_eq!(drifts, [drift("a", 5, 4), drift("c", 0, 1), drift("d", 2, 0)]);
    assert_eq!(drifts.iter().map(Drift::above).collect::<Vec<_>>(), [true, false, true]);
}

#[test]
fn a_blessed_baseline_reads_back() {
    let root = Scratch::new("comments-bless");
    let held = counts(&[("asm", 3), ("base", 0)]);
    held.bless(root.path()).unwrap();
    assert_eq!(Counts::baseline(root.path()).unwrap(), held);
    assert_eq!(Counts::from_source(&held.source().unwrap()).unwrap(), held);
}

#[test]
fn the_repository_is_within_its_baseline() {
    let root = vun_harness::repo_root();
    let drifts = Counts::of(&root).unwrap().drift(&Counts::baseline(&root).unwrap());
    assert!(drifts.is_empty(), "{drifts:?}; run `cargo xtask comments`");
}
