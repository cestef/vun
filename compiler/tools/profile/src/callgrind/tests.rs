use vun_intern::Symbol;

use super::{Profile, Reason, Source};

/// `main` calls `hot` twice and recurses once into itself; `hot` runs inlined code of `blake3`.
const PROFILE: &str = "\
positions: line
events: Ir

ob=(1) /bin/vun
fl=(1) vun-build/vun/compiler/base/query/src/lib.rs
fn=(1) main
10 5
cfn=(2) hot
calls=2 3
* 40
cfn=(3) main'2
calls=1 3
* 7
fn=(3) main'2
10 7
fl=(2) vun-build/vun/compiler/front/syntax/src/lexer.rs
fn=(2) hot
3 20
fi=(3) /home/u/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/blake3-1.8.7/src/portable.rs
4 15
fe=(2)
5 5
";

fn query() -> Source {
    Source::Workspace { group: Symbol::intern("base"), name: Symbol::intern("query") }
}

#[test]
fn functions_hold_their_own_and_inclusive_instructions() {
    let profile = Profile::parse(PROFILE).unwrap();
    let rows: Vec<_> =
        profile.functions.iter().map(|f| (f.name.as_str(), f.own, f.inclusive, f.calls)).collect();
    assert_eq!(rows, [("hot", 40, Some(40), 2), ("main", 12, Some(52), 1)]);
    assert_eq!(profile.total, 52);
}

#[test]
fn inlined_code_counts_where_it_is_written() {
    let profile = Profile::parse(PROFILE).unwrap();
    let blake3 = Source::Registry(Symbol::intern("blake3"));
    assert_eq!(profile.share_of(blake3), 15);
    assert_eq!(profile.share_of(query()), 12);
}

#[test]
fn a_recursion_level_is_the_function_and_calls_to_itself_are_no_edge() {
    let profile = Profile::parse(PROFILE).unwrap();
    assert_eq!(profile.edges.len(), 1);
    let edge = profile.edges[0];
    assert_eq!((profile.functions[edge.caller].name.as_str(), edge.calls, edge.cost), ("main", 2, Some(40)));
    assert_eq!(profile.callers(edge.callee), [edge]);
}

#[test]
fn a_profile_that_does_not_count_instructions_is_refused() {
    let error = Profile::parse("events: Dr\n").unwrap_err();
    assert_eq!((error.line, error.reason), (1, Reason::Event));
}

#[test]
fn sources_are_read_off_paths() {
    let toolchain = Source::of("/rustc/48a2/library/core/src/num/uint_macros.rs", "vun");
    assert_eq!(toolchain, Source::Toolchain(Symbol::intern("core")));
    assert_eq!(
        Source::of("/usr/src/glibc/memcpy.S", "/usr/lib64/libc.so.6"),
        Source::System(Symbol::intern("libc.so.6"))
    );
}

/// `a` and `b` call each other, `a` is called from `main` and `b` calls `leaf`.
const CYCLE: &str = "\
positions: line
events: Ir

fl=(1) vun-build/vun/compiler/base/query/src/lib.rs
fn=(1) main
1 1
cfn=(2) a
calls=1 1
* 37
fn=(2) a
1 10
cfn=(3) b
calls=5 1
* 27
fn=(3) b
1 20
cfn=(2) a
calls=4 1
* 17
cfn=(4) leaf
calls=1 1
* 7
fn=(4) leaf
1 7
";

#[test]
fn functions_that_reach_themselves_through_others_have_no_inclusive_cost() {
    let profile = Profile::parse(CYCLE).unwrap();
    let inclusive: Vec<_> = profile.functions.iter().map(|f| (f.name.as_str(), f.inclusive)).collect();
    assert_eq!(inclusive, [("b", None), ("a", None), ("leaf", Some(7)), ("main", Some(38))]);
    assert_eq!(profile.edges.iter().filter(|e| e.cost.is_none()).count(), 2);
}
