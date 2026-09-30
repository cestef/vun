//! Tests run by `std.testing.runner` (spec 11): each on a thread of its own, a trap ending it
//! alone, one line each and then the totals, read back as typed values.

use vun_build::layout::MANIFEST;
use vun_driver::Codegen;
use vun_driver::testing::{Line, Outcome, Place, Ran, Said, Totals, Trapped};

use crate::backend::BACKENDS;
use crate::common::Fixture;

/// The id of the first package of these tests, which no other test's package has.
const SUITE: u64 = 1100;

/// The plan every package declares, here empty.
const BUILD: &str = "build := (b: &mut std.build.Build) {};\n";

/// Two tests of the root file, reading a global only run time knows, and two of a namespace
/// below, one reading past an array.
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
        let index = Said { message: None, at: Some(at(&more, 4, 8)) };
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
            Line::Totals(Totals { passed: 2, trapped: 2 }),
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
