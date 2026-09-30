//! Fuzzing (spec 11, section 2): a program built covered marks each block it runs, and fuzzing a
//! property keeps the cases reaching new blocks, so it goes past tests a case drawn at random
//! passes only once in 2^24.

use vun_build::layout::MANIFEST;
use vun_driver::testing::{Argument, Mode, Options, Outcome};
use vun_sema::cover::Coverage;
use vun_term::Shell;

use crate::common::Fixture;
use crate::testing::BUILD;

/// The id of this test's package, which no other test's package has.
const PACKAGE: u64 = 1140;

/// A trap behind three tests of one byte each, a branch each.
const NESTED: &str = "\
#[test]
deep := (a: u8, b: u8, c: u8) {
    if a == 7 {
        if b == 9 {
            if c == 11 { assert(false); }
        }
    }
};
";

#[test]
fn fuzzing_a_covered_program_reaches_what_random_cases_do_not() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let fixture = Fixture::new("testing-fuzz");
    let mut app = Fixture::manifest("fuzzed", PACKAGE, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write("", &app, NESTED);
    let manifest = fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
    let mut session = fixture.session();
    session.cover(Coverage::Blocks);
    let file = session.load(&manifest).unwrap();
    let ns = session.tested(file, Some(fixture.root.path())).unwrap();
    let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let run = |options: Options| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let run = session.run_tests(&program, [""; 0], options).unwrap();
        let reported = run.report(&mut Shell::plain(&mut out, &mut err)).unwrap();
        let mut tests = reported.tests.into_iter();
        let tested = tests.next().unwrap();
        (tested.ran.outcome, tested.arguments)
    };
    let (drawn, _) = run(Options { cases: Some(20_000), ..Options::default() });
    assert_eq!(drawn, Outcome::Passed);
    let (fuzzed, arguments) = run(Options { cases: Some(20_000), mode: Mode::Fuzz, ..Options::default() });
    assert!(matches!(fuzzed, Outcome::Falsified(_)), "{fuzzed:#?}");
    let expected = [("a", "7"), ("b", "9"), ("c", "11")].map(|(name, json)| Argument {
        name: name.to_owned(),
        value: vun_data::json::parse(json).unwrap().into_owned(),
    });
    assert_eq!(arguments, expected);
}
