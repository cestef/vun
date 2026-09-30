//! Simulations (spec 11, section 4): tasks run on a virtual clock, one at a time as the case's
//! choices schedule them, so a race is found over the cases a property runs, and replayed.

use vun_build::layout::MANIFEST;
use vun_driver::testing::{Falsified, Options, Outcome};
use vun_term::Shell;

use crate::common::Fixture;
use crate::testing::BUILD;

/// The id of this test's package, which no other test's package has.
const PACKAGE: u64 = 1130;

/// Two tasks sleeping on the clock, which moves only when neither is ready, and two racing: the
/// one writing last wins in some schedules, which the assertion forbids.
const SIMULATIONS: &str = "\
/// Adds `by` to `total` after sleeping `wait` milliseconds, twice.
worker := (clock: &std.testing.Clock, total: &std.sync.Atomic(u32), by: u32, wait: u64) -> std.task.Async(()) {
    for _ in 0..2 {
        yield ..clock.sleep(std.time.Duration.millis(wait));
        _ = total.add(by, .relaxed);
    }
};

#[test]
sums := (s: &mut std.testing.Sim) {
    clock := std.testing.Clock.init();
    total := std.sync.Atomic(u32).init(0);
    mut a := worker(&clock, &total, 1, 10);
    mut b := worker(&clock, &total, 100, 15);
    ended := s.run(&clock, .{ &mut a, &mut b }, std.time.Duration.seconds(1));
    a.end();
    b.end();
    assert(ended == .done);
    assert(total.load(.relaxed) == 202);
    assert(clock.now().as_nanos() == 30 * std.time.nanos_per_milli);
};

/// Writes `mine` into `slot` unless another wrote first, over a sleep: a race the schedule decides.
racer := (clock: &std.testing.Clock, slot: &std.sync.Atomic(u32), mine: u32) -> std.task.Async(()) {
    seen := slot.load(.relaxed);
    yield ..clock.sleep(std.time.Duration.millis(1));
    if seen == 0 { slot.store(mine, .relaxed); }
};

#[test]
races := (s: &mut std.testing.Sim) {
    clock := std.testing.Clock.init();
    slot := std.sync.Atomic(u32).init(0);
    mut a := racer(&clock, &slot, 1);
    mut b := racer(&clock, &slot, 2);
    _ = s.run(&clock, .{ &mut a, &mut b }, std.time.Duration.seconds(1));
    a.end();
    b.end();
    assert(slot.load(.relaxed) == 1);
};
";

#[test]
fn a_simulation_schedules_its_tasks_by_its_choices_and_finds_a_race() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let fixture = Fixture::new("testing-sim");
    let mut app = Fixture::manifest("simmed", PACKAGE, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fixture.write("", &app, SIMULATIONS);
    let manifest = fixture.root.write(dir.join(MANIFEST), [app.source().unwrap().as_str(), BUILD].concat());
    let mut session = fixture.session();
    let file = session.load(&manifest).unwrap();
    let ns = session.tested(file, Some(fixture.root.path())).unwrap();
    let program = session.build_tests(file, ns, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let outcomes = |options: Options| {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let run = session.run_tests(&program, [""; 0], options).unwrap();
        let reported = run.report(&mut Shell::plain(&mut out, &mut err)).unwrap();
        let tests = reported.tests.into_iter();
        tests.map(|tested| (tested.ran.name, tested.ran.outcome, tested.arguments.len())).collect::<Vec<_>>()
    };
    let first = outcomes(Options::default());
    let [(sums, Outcome::Passed, 0), (races, Outcome::Falsified(Falsified { seed, case, .. }), 0)] =
        first.as_slice()
    else {
        panic!("{first:#?}")
    };
    assert_eq!((sums.as_str(), races.as_str()), ("sums", "races"));
    // The seed a failure reports replays it: the same case fails first.
    let replayed = outcomes(Options { seed: Some(*seed), ..Options::default() });
    assert!(
        matches!(&replayed[1].1, Outcome::Falsified(f) if f.case == *case && f.seed == *seed),
        "{replayed:#?}"
    );
}
