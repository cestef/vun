//! Benchmarks (spec 11, section 5): run as tests, each loop runs its body once; measured, the
//! benchmarks alone run, each until its loop has taken `std.testing.bench.least`.

use vun_build::layout::MANIFEST;
use vun_driver::testing::{Baseline, Measure, Measured, Mode, Options, Outcome, Plan};
use vun_term::Percent;
use vun_term::Shell;

use crate::support::Fixture;
use crate::testing::BUILD;

/// The id of this test's package, which no other test's package has.
const PACKAGE: u64 = 1120;

/// `std.testing.bench.least`: how long a measured loop runs at least.
const LEAST_NANOS: u64 = 500_000_000;

const BENCHMARKS: &str = "\
#[test]
sums := (b: &mut std.testing.Bench) {
    xs: [64]u32 = [3; 64];
    for b.loop() {
        mut total: u32 = 0;
        for i in 0..64 { total +%= xs[i]; }
        b.keep(&total);
    }
};

#[test]
adds := () { assert(1 + 1 == 2); };
";

#[test]
fn a_benchmark_runs_once_as_a_test_and_alone_when_measured() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let fixture = Fixture::new("testing-bench");
    let mut app = Fixture::manifest("benched", PACKAGE, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write("", &app, BENCHMARKS);
    let manifest = fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
    let mut session = fixture.session();
    let file = session.load(&manifest).unwrap();
    let ns = session.tested(file, Some(fixture.root.path())).unwrap();
    let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let outcomes = |options: Options| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let run = session.run_tests(&program, [""; 0], options).unwrap();
        let reported = run.report(&mut Shell::plain(&mut out, &mut err)).unwrap();
        reported.tests.into_iter().map(|tested| (tested.ran.name, tested.ran.outcome)).collect::<Vec<_>>()
    };
    let tested = outcomes(Options::default());
    assert_eq!(tested, [("sums".to_owned(), Outcome::Passed), ("adds".to_owned(), Outcome::Passed)]);
    let measured = outcomes(Options { mode: Mode::Bench, ..Options::default() });
    let [(name, Outcome::Measured(Measured { iterations, nanos }))] = measured.as_slice() else {
        panic!("{measured:#?}")
    };
    assert_eq!(name, "sums");
    assert!(*iterations > 1 && *nanos >= LEAST_NANOS, "{measured:#?}");
}

fn plan(measure: Measure, save: bool, fail_over: Option<Percent>) -> Plan {
    Plan { measure, samples: 2, baseline: "base".to_owned(), save, fail_over }
}

#[test]
fn benchmarks_are_sampled_kept_as_a_baseline_and_compared_with_it() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let fixture = Fixture::new("testing-bench-baseline");
    let mut app = Fixture::manifest("benched", PACKAGE + 1, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write("", &app, BENCHMARKS);
    let manifest = fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
    let mut session = fixture.session();
    let file = session.load(&manifest).unwrap();
    let ns = session.tested(file, Some(fixture.root.path())).unwrap();
    let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let bench = |plan: Plan| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let passed =
            session.bench(&program, &dir, &[], &plan, &mut Shell::plain(&mut out, &mut err)).unwrap();
        (passed, String::from_utf8(out).unwrap() + &String::from_utf8(err).unwrap())
    };
    assert_eq!(Baseline::read(&dir, "base").unwrap(), None);
    let (passed, _) = bench(plan(Measure::Time, true, None));
    assert!(passed);
    let kept = Baseline::read(&dir, "base").unwrap().unwrap();
    assert_eq!(kept.measure, Measure::Time);
    let statistics = kept.benchmarks["sums"];
    assert_eq!(kept.benchmarks.len(), 1);
    assert_eq!(statistics.samples, 2);
    assert!(statistics.median > 0.0, "{statistics:?}");
    // The same program, a threshold no noise reaches.
    let (passed, _) = bench(plan(Measure::Time, false, Some(Percent(1000.0))));
    assert!(passed);
    // Instructions are counted where `perf` counts them, else the run is timed and says why.
    let (passed, said) = bench(plan(Measure::Instructions, true, None));
    assert!(passed, "{said}");
    let kept = Baseline::read(&dir, "base").unwrap().unwrap();
    assert!(kept.benchmarks.contains_key("sums"), "{kept:?}");
}
