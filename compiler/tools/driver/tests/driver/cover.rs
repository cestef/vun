//! `vun cover` (spec 11, section 6): the tests of a namespace run in a program built covered, and
//! the flags it writes are read against the blocks of the program, by line and by function.

use vun_build::layout::MANIFEST;
use vun_driver::cover::Lcov;
use vun_driver::testing::{Mode, Options};
use vun_sema::cover::Coverage;
use vun_term::Shell;

use crate::common::Fixture;
use crate::testing::BUILD;

/// The id of this test's package, which no other test's package has.
const PACKAGE: u64 = 1150;

/// A function whose test takes one branch, and a function no test calls.
const SOURCE: &str = "\
pick := (n: u32) -> u32 {
    if n > 10 {
        n - 10
    } else {
        n + 1
    }
};

unused := (n: u32) -> u32 {
    n % 7
};

#[test]
low := () { assert(pick(3) == 4); };
";

/// The lines of `SOURCE` holding the branch the test takes, and the ones it does not.
const TAKEN: u32 = 5;
const MISSED: [u32; 2] = [3, 10];

#[test]
fn a_covered_run_reaches_the_lines_and_functions_its_tests_run() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let fixture = Fixture::new("testing-cover");
    let mut app = Fixture::manifest("covered", PACKAGE, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write("", &app, SOURCE);
    let manifest = fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
    let root = Fixture::file(&dir, &app);
    let mut session = fixture.session();
    session.cover(Coverage::Blocks);
    let file = session.load(&manifest).unwrap();
    let ns = session.tested(file, Some(fixture.root.path())).unwrap();
    let program =
        session.build_tests(file, ns.clone(), None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let options = Options { mode: Mode::Cover, ..Options::default() };
    let run = session.run_tests(&program, [""; 0], options).unwrap();
    let reported = run.report(&mut Shell::plain(&mut out, &mut err)).unwrap();
    assert!(reported.passed);
    let covered = reported.covered.expect("the program wrote its flags");
    let tested = session.tests_program(file, ns).unwrap();
    let reach = session.reached(&tested, &covered, fixture.root.path(), std::slice::from_ref(&root));
    let source = reach.files.iter().find(|(path, _)| path.ends_with(root.file_name().unwrap())).unwrap().1;
    assert_eq!(source.lines.get(&TAKEN), Some(&true), "{source:#?}");
    for line in MISSED {
        assert_eq!(source.lines.get(&line), Some(&false), "line {line}: {source:#?}");
    }
    // `pick` is called, `unused` is not, and the test is a function the program runs.
    let called: Vec<(u32, bool)> = source.functions.iter().map(|(&(line, _), &hit)| (line, hit)).collect();
    assert_eq!(called, [(1, true), (9, false), (14, true)]);
    let lcov = Lcov(&reach).to_string();
    assert!(lcov.contains(&format!("DA:{TAKEN},1\n")), "{lcov}");
    assert!(lcov.contains("DA:10,0\n") && lcov.contains("FNF:3\nFNH:2\n"), "{lcov}");
}
