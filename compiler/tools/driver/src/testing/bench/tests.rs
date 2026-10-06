use vun_fs::Scratch;
use vun_term::Percent;

use super::baseline::BaselineError;
use super::counter::Counter;
use super::{Baseline, Comparison, Measure, Statistics, Verdict};

fn statistics(values: &[f64]) -> Statistics {
    Statistics::of(values).unwrap()
}

#[test]
fn the_median_and_the_spread_are_not_moved_by_an_outlier() {
    let steady = statistics(&[10.0, 11.0, 10.0, 12.0, 11.0, 10.0, 11.0]);
    let noisy = statistics(&[10.0, 11.0, 10.0, 12.0, 11.0, 10.0, 11.0, 400.0]);
    assert_eq!(steady.outliers, 0);
    assert_eq!(noisy.outliers, 1);
    assert!((noisy.median - 11.0).abs() < 1.0, "{noisy:?}");
    assert!(noisy.high < 20.0 && noisy.mean < 20.0, "{noisy:?}");
    assert!(noisy.deviation <= 1.0, "{noisy:?}");
    assert_eq!(noisy.samples, 8);
}

#[test]
fn no_samples_have_no_statistics_and_one_has_no_spread() {
    assert_eq!(Statistics::of(&[]), None);
    let one = statistics(&[5.0]);
    assert_eq!((one.median, one.mean, one.low, one.high, one.deviation), (5.0, 5.0, 5.0, 5.0, 0.0));
    assert_eq!(one.spread(), Percent(0.0));
}

#[test]
fn a_change_is_a_regression_past_the_threshold_and_the_noise() {
    let base = statistics(&[100.0, 100.0, 100.0]);
    let slower = statistics(&[104.0, 104.0, 104.0]);
    let at = |now: &Statistics, over| Comparison::of(now, &base, over);
    assert_eq!(at(&slower, None).verdict, Verdict::Regressed);
    assert_eq!(at(&slower, Some(Percent(3.0))).verdict, Verdict::Regressed);
    assert_eq!(at(&slower, Some(Percent(5.0))).verdict, Verdict::Unchanged);
    assert!(at(&slower, Some(Percent(3.0))).fails(Some(Percent(3.0))));
    assert!(!at(&slower, None).fails(None));
    assert_eq!(at(&statistics(&[90.0, 90.0, 90.0]), Some(Percent(3.0))).verdict, Verdict::Improved);
    let noisy = statistics(&[90.0, 100.0, 110.0, 100.0, 120.0]);
    assert_eq!(at(&noisy, None).verdict, Verdict::Unchanged);
}

#[test]
fn a_baseline_is_kept_and_read_back_with_the_benchmarks_it_had() {
    let dir = Scratch::new("baseline");
    let one = |name: &str, value: f64| Baseline {
        measure: Measure::Time,
        benchmarks: [(name.to_owned(), statistics(&[value, value + 1.0]))].into(),
    };
    assert_eq!(Baseline::read(dir.path(), "base").unwrap(), None);
    let path = one("a", 1.0).write(dir.path(), "base").unwrap();
    assert!(path.starts_with(dir.path()) && path.ends_with(".vun/bench/base.json"), "{path:?}");
    one("b", 7.0).write(dir.path(), "base").unwrap();
    let read = Baseline::read(dir.path(), "base").unwrap().unwrap();
    assert_eq!(read.benchmarks.keys().collect::<Vec<_>>(), ["a", "b"]);
    assert_eq!(read.benchmarks["b"], statistics(&[7.0, 8.0]));
    let other = Baseline { measure: Measure::Instructions, ..one("c", 2.0) };
    other.write(dir.path(), "base").unwrap();
    let read = Baseline::read(dir.path(), "base").unwrap().unwrap();
    assert_eq!((read.measure, read.benchmarks.len()), (Measure::Instructions, 1));
}

#[test]
fn a_baseline_name_is_a_file_name() {
    for name in ["", "..", "a/b", "/etc/x"] {
        assert!(
            matches!(Baseline::path(std::path::Path::new("."), name), Err(BaselineError::Name(_))),
            "{name}"
        );
    }
}

#[test]
fn the_count_is_the_first_field_of_the_event_line() {
    let counted = "# started on Wed Oct  7 00:39:07 2026\n\n123456789,,instructions:u,500000,100.00,,\n";
    assert_eq!(Counter::parse(counted), Some(123_456_789));
    assert_eq!(Counter::parse("<not supported>,,instructions:u,0,100.00,,\n"), None);
    assert_eq!(Counter::parse("Error:\nNo supported events found.\n"), None);
}

#[test]
fn a_measure_is_named_by_what_the_option_takes() {
    for measure in Measure::ALL {
        assert_eq!(Measure::named(measure.name()), Some(*measure));
    }
    assert_eq!(Measure::named("cycles"), None);
}
