//! Tests run by `std.testing.runner` (spec 11): each on a thread of its own, a trap ending it
//! alone, one line each and then the totals, read back as typed values.

use vun_build::layout::MANIFEST;
use vun_driver::Codegen;
use vun_driver::testing::{Argument, Ignored, Line, Options, Outcome, Place, Ran, Said, Totals, Trapped};
use vun_term::Shell;

use crate::backend::BACKENDS;
use crate::common::Fixture;

/// The id of the first package of these tests, which no other test's package has.
const SUITE: u64 = 1100;

/// The plan every package declares, here empty.
pub(crate) const BUILD: &str = "build := (b: &mut std.build.Build) {};\n";

/// Two tests of the root file, reading a global only run time knows, and three of a namespace
/// below, one reading past an array and one not run.
const ROOT: &str = "\
three: std.sync.Atomic(u32) = .init(3);

#[test]
adds := () { assert(three.load(.relaxed) + 1 == 4); };

#[test]
fails := () { assert(three.load(.relaxed) == 4); };

untested := () {};
";

const MORE: &str = "\
#[test]
indexes := () {
    items: [2]u8 = [1, 2];
    if items[suite.three.load(.relaxed).to(usize)] == 1 {}
};

#[test]
last := () {};

#[test]
#[test.ignore(\"not yet\")]
skipped := () { assert(false); };
";

fn ran(name: &str, outcome: Outcome) -> Line {
    Line::Test(Ran { name: name.to_owned(), outcome })
}

#[test]
fn the_runner_runs_each_test_on_a_thread_and_reports_its_trap() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for (id, backend) in BACKENDS.into_iter().enumerate() {
        let fixture = Fixture::new(&format!("testing-runner-{backend}"));
        // A package of its own for each backend, and apart from other tests' packages: origins
        // of one identity are one in a process.
        let mut app = Fixture::manifest("suite", u64::try_from(id).unwrap() + SUITE, "0.1.0");
        app.include.push(app.name.clone());
        let dir = fixture.write("", &app, ROOT);
        // The tests' program is built from the manifest, whose plan runs.
        let manifest =
            fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
        let root = Fixture::file(&dir, &app);
        let more = fixture.root.write("suite/more.vun", MORE);
        let mut session = fixture.session();
        session.codegen = Codegen::through(backend);
        let file = session.load(&manifest).unwrap();
        let ns = session.tested(file, Some(fixture.root.path())).unwrap();
        let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
        let run = std::process::Command::new(program).output().unwrap();
        let lines: Vec<Line> = String::from_utf8(run.stdout).unwrap().lines().filter_map(Line::of).collect();
        let at =
            |file: &std::path::Path, line, column| Place { file: file.display().to_string(), line, column };
        let assertion = Said {
            message: Some("assertion failed: `three.load(.relaxed) == 4`".to_owned()),
            at: Some(at(&root, 7, 22)),
        };
        let index =
            Said { message: Some("an index past the last item".to_owned()), at: Some(at(&more, 4, 8)) };
        let trapped = |said: &Said| Outcome::Trapped(Trapped { site: 0, said: Some(said.clone()) });
        // Site ids are the program's own: compared apart from them.
        let sites: Vec<Line> = lines
            .into_iter()
            .map(|line| match line {
                Line::Test(Ran { name, outcome: Outcome::Trapped(t) }) => {
                    Line::Test(Ran { name, outcome: Outcome::Trapped(Trapped { site: 0, ..t }) })
                }
                line => line,
            })
            .collect();
        let expected = [
            ran("adds", Outcome::Passed),
            ran("fails", trapped(&assertion)),
            ran("indexes", trapped(&index)),
            ran("last", Outcome::Passed),
            ran("skipped", Outcome::Ignored(Ignored { reason: "not yet".to_owned() })),
            Line::Totals(Totals { passed: 2, trapped: 2, falsified: 0, ignored: 1, measured: 0 }),
        ];
        assert_eq!(sites, expected, "{backend}");
        assert_eq!(run.status.code(), Some(1), "{backend}");
    }
}

#[test]
fn a_file_outside_any_package_names_no_tests() {
    let fixture = Fixture::new("testing-loose");
    let path = fixture.root.write("loose.vun", "#[test]\nok := () {};\n");
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    assert_eq!(session.tested(file, Some(&path)), None);
    let reported: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(reported, [vun_diag::errors::TEST_NO_NAMESPACE]);
}

/// Properties over bytes, a signed byte, a struct, a tuple, an array, a float and a slice: two
/// holding, one of them over a generator a parameter names, and the others falsified, each shrunk
/// to the least arguments that fail.
const PROPERTIES: &str = "\
Pair := struct { a: u8, flag: bool };

digit := (c: &mut std.testing.Choices) -> u8 { c.draw(9).to(u8) };

#[test]
holds := (x: u8, y: u8) { assert(x + y <= 510); };

#[test]
digits := (#[test.gen(digit)] d: u8) { assert(d < 10); };

#[test]
bounded := (x: u8) { assert(x < 200); };

#[test]
signed := (n: i8) { assert(n >= -100); };

#[test]
paired := (p: Pair) { assert(!(p.flag and p.a > 10)); };

#[test]
tupled := (t: (u8, bool)) { assert(!(t.1 and t.0 > 5)); };

#[test]
arrays := (xs: [3]u16) { assert(xs[1] < 100); };

#[test]
floats := (x: f32) { assert(!(x > 1.0)); };

#[test]
fuzzed := (input: &[u8]) { assert(input.len() < 3); };
";

#[test]
fn a_property_runs_over_generated_arguments_and_shrinks_the_case_that_fails() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for (id, backend) in BACKENDS.into_iter().enumerate() {
        let fixture = Fixture::new(&format!("testing-properties-{backend}"));
        let mut app = Fixture::manifest("props", u64::try_from(id).unwrap() + SUITE + 10, "0.1.0");
        app.include.push(app.name.clone());
        let dir = fixture.write("", &app, PROPERTIES);
        let manifest =
            fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
        let mut session = fixture.session();
        session.codegen = Codegen::through(backend);
        let file = session.load(&manifest).unwrap();
        let ns = session.tested(file, Some(fixture.root.path())).unwrap();
        let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
        let run = std::process::Command::new(&program).output().unwrap();
        let stdout = String::from_utf8(run.stdout).unwrap();
        let lines: Vec<Line> = stdout.lines().filter_map(Line::of).collect();
        // What each test's outcome says, apart from the program's site ids and the cases drawn.
        let said: Vec<(String, Option<String>)> = lines
            .iter()
            .filter_map(|line| match line {
                Line::Test(Ran { name, outcome }) => {
                    Some((name.clone(), outcome.trap().and_then(|t| t.said.clone()?.message)))
                }
                Line::Totals(_) | Line::Argument(_) => None,
            })
            .collect();
        let failed =
            |name: &str, condition: &str| (name.to_owned(), Some(format!("assertion failed: `{condition}`")));
        let expected = [
            ("holds".to_owned(), None),
            ("digits".to_owned(), None),
            failed("bounded", "x < 200"),
            failed("signed", "n >= -100"),
            failed("paired", "!(p.flag and p.a > 10)"),
            failed("tupled", "!(t.1 and t.0 > 5)"),
            failed("arrays", "xs[1] < 100"),
            failed("floats", "!(x > 1.0)"),
            failed("fuzzed", "input.len() < 3"),
        ];
        assert_eq!(said, expected, "{backend}");
        let totals = lines.iter().find_map(|l| match l {
            Line::Totals(totals) => Some(*totals),
            Line::Test(_) | Line::Argument(_) => None,
        });
        assert_eq!(
            totals,
            Some(Totals { passed: 2, trapped: 0, falsified: 7, ignored: 0, measured: 0 }),
            "{backend}"
        );
        // The smallest failing arguments, each a line before its test's.
        let written: Vec<&Argument> = lines
            .iter()
            .filter_map(|l| match l {
                Line::Argument(argument) => Some(argument),
                Line::Test(_) | Line::Totals(_) => None,
            })
            .collect();
        let shrunk = [
            ("x", "200"),
            ("n", "-101"),
            ("p", "{\"a\":11,\"flag\":true}"),
            ("t", "[6,true]"),
            ("xs", "[0,100,0]"),
            ("x", "\"0x1.000002p+0\""),
            ("input", "\"\\u0000\\u0000\\u0000\""),
        ]
        .map(|(name, json)| Argument {
            name: name.to_owned(),
            value: vun_data::json::parse(json).unwrap().into_owned(),
        });
        assert_eq!(written, shrunk.iter().collect::<Vec<_>>(), "{backend}");
        assert_eq!(run.status.code(), Some(1), "{backend}");
        // A run asking for a seed reports it; one asking for no cases falsifies nothing.
        let outcomes = |names: &[&str], options: Options| {
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let run = session.run_tests(&program, names, options).unwrap();
            let reported = run.report(&mut Shell::plain(&mut out, &mut err)).unwrap();
            reported.tests.into_iter().map(|tested| tested.ran.outcome).collect::<Vec<_>>()
        };
        let seeded = outcomes(&["bounded"], Options { seed: Some(7), cases: None, bench: false });
        assert!(
            matches!(seeded.as_slice(), [Outcome::Falsified(f)] if f.seed == 7),
            "{backend}: {seeded:#?}"
        );
        let none = outcomes(&[], Options { seed: None, cases: Some(0), bench: false });
        assert!(none.len() == 9 && none.iter().all(|o| *o == Outcome::Passed), "{backend}: {none:#?}");
    }
}
