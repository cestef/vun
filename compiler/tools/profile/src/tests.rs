use std::time::Duration;

use vun_driver::testing::{Comparison, Measure, Statistics, Verdict};
use vun_fs::Scratch;
use vun_trace::Kind;

use super::{Entries, Entry, Key, Phase, Sample, Scope, Setup, Snapshot, Timed, Verb, Weight, Workload};

const OUTPUT: &str = r#"{"type":"started","step":"check","subject":"a.vun"}
{"type":"timings","wall_us":5000,"work_us":4000,"activities":[{"kind":"query","name":"parsed","runs":3,"own_us":3000,"total_us":3000,"longest_us":2000},{"kind":"stage","name":"save memos","runs":1,"own_us":1000,"total_us":1000,"longest_us":1000},{"kind":"nothing","name":"x","runs":1,"own_us":1,"total_us":1,"longest_us":1}]}
{"type":"threads","threads":1}
"#;

fn sample(wall_ms: u64, parsed_ms: u64) -> Sample {
    let ms = Duration::from_millis;
    Sample {
        wall: ms(wall_ms),
        work: ms(parsed_ms),
        activities: vec![Timed {
            kind: Kind::Query,
            name: "parsed".to_owned(),
            runs: 3,
            own: ms(parsed_ms),
            total: ms(parsed_ms),
        }],
    }
}

fn entries(workload: &Workload) -> Entries<'_> {
    Entries { workload, phase: Phase::Warm }
}

#[test]
fn the_timings_record_is_found_among_the_lines_and_unknown_kinds_are_left_out() {
    let sample = Sample::read(OUTPUT).unwrap();
    let rows: Vec<_> = sample.activities.iter().map(|a| (a.kind, a.name.as_str(), a.runs, a.own)).collect();
    assert_eq!(
        rows,
        [
            (Kind::Query, "parsed", 3, Duration::from_millis(3)),
            (Kind::Stage, "save memos", 1, Duration::from_millis(1)),
        ]
    );
    assert_eq!((sample.wall, sample.work), (Duration::from_millis(5), Duration::from_millis(4)));
}

#[test]
fn output_without_timings_is_refused() {
    assert!(matches!(Sample::read("{\"type\":\"started\"}\n"), Err(crate::MeasureError::Unreported)));
}

#[test]
fn samples_give_each_activity_the_statistics_of_its_own_times() {
    let workload = &Workload::ALL[0];
    let kept = entries(workload).of_samples(&[sample(10, 4), sample(12, 6), sample(11, 5)]);
    let parsed = kept.iter().find(|e| e.key.scope == Scope::Activity).unwrap();
    assert_eq!(parsed.key.name, "query parsed");
    assert_eq!(parsed.key.measure, Measure::Time);
    assert!((parsed.statistics.median - 5e6).abs() < 1.0);
    let wall = kept.iter().find(|e| e.key.scope == Scope::Total).unwrap();
    assert!((wall.statistics.median - 11e6).abs() < 1.0);
}

fn key(name: &str) -> Key {
    Key {
        workload: "check-tour".to_owned(),
        phase: Phase::Cold,
        measure: Measure::Instructions,
        scope: Scope::Function,
        name: name.to_owned(),
    }
}

fn entry(name: &str, value: f64) -> Entry {
    Entry { key: key(name), statistics: Statistics::of(&[value]).unwrap() }
}

#[test]
fn a_snapshot_keeps_what_a_later_one_does_not_measure_again() {
    let scratch = Scratch::new("profile-snapshot");
    Snapshot::write(vec![entry("a", 10.0), entry("b", 20.0)], scratch.path(), "base").unwrap();
    Snapshot::write(vec![entry("b", 25.0)], scratch.path(), "base").unwrap();
    let kept = Snapshot::read(scratch.path(), "base").unwrap().unwrap();
    assert_eq!(kept.find(&key("a")), Some(&entry("a", 10.0)));
    assert_eq!(kept.find(&key("b")), Some(&entry("b", 25.0)));
    assert_eq!(kept.entries.len(), 2);
}

#[test]
fn a_missing_snapshot_is_none_and_a_name_with_a_separator_is_refused() {
    let scratch = Scratch::new("profile-missing");
    assert_eq!(Snapshot::read(scratch.path(), "none").unwrap(), None);
    assert!(Snapshot::read(scratch.path(), "a/b").is_err());
}

#[test]
fn instructions_within_the_tolerance_are_unchanged_and_past_it_regressed() {
    let (base, same, worse) = (entry("a", 1000.0), entry("a", 1004.0), entry("a", 1100.0));
    let tolerance = Some(vun_term::Percent(0.5));
    let verdict = |now: &Entry| Comparison::of(&now.statistics, &base.statistics, tolerance).verdict;
    assert_eq!((verdict(&same), verdict(&worse)), (Verdict::Unchanged, Verdict::Regressed));
}

#[test]
fn a_workload_is_copied_whether_it_is_a_file_or_a_package() {
    let root = Scratch::new("profile-root");
    root.write("one.vun", "a");
    root.write("pkg/build.vun", "b");
    root.write("pkg/sub/x.vun", "c");
    let copied = |path: &'static str| {
        let workload = Workload { name: "w", path, verb: Verb::Check, weight: Weight::Light };
        Setup::new(std::path::Path::new("vun"), root.path(), &workload, None, None).unwrap()
    };
    assert!(copied("one.vun").program().is_file());
    let package = copied("pkg");
    assert!(package.program().join("build.vun").is_file() && package.program().join("sub/x.vun").is_file());
}

#[test]
fn a_built_program_has_one_phase_whatever_is_asked() {
    let program = Workload::named("run-fib").unwrap();
    let compiled = Workload::named("check-hello").unwrap();
    assert_eq!(program.phases(Some(Phase::Cold)), vec![Phase::Run]);
    assert_eq!(compiled.phases(None), vec![Phase::Cold, Phase::Warm]);
    assert_eq!(compiled.phases(Some(Phase::Warm)), vec![Phase::Warm]);
}
