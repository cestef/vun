use std::os::unix::process::ExitStatusExt;
use std::process::{ExitStatus, Output};
use std::time::Duration;

use super::{Difference, Ran, Stream};

fn ran(code: i32, stdout: &str, stderr: &str) -> Ran {
    let output = Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    };
    Ran { backend: "test", output, size: 0, built: Duration::ZERO, runs: vec![Duration::ZERO] }
}

#[test]
fn runs_that_end_and_write_alike_agree() {
    let runs = [ran(0, "a\nb\n", "w"), ran(0, "a\nb\n", "w")];
    assert!(Ran::agree(&runs));
    assert_eq!(runs[1].differs(&runs[0]), None);
}

#[test]
fn the_exit_code_parts_runs_before_what_they_wrote() {
    let (reference, other) = (ran(0, "a", ""), ran(3, "b", ""));
    assert_eq!(other.differs(&reference), Some(Difference::Exit { reference: Some(0), other: Some(3) }));
}

#[test]
fn the_first_line_that_differs_is_named() {
    let (reference, other) = (ran(0, "a\nb\nc\n", ""), ran(0, "a\nx\nc\n", ""));
    assert_eq!(
        other.differs(&reference),
        Some(Difference::Output {
            stream: Stream::Stdout,
            line: 2,
            reference: Some("b".to_owned()),
            other: Some("x".to_owned()),
        })
    );
}

#[test]
fn a_stream_that_ends_early_has_no_line_where_it_did() {
    let (reference, other) = (ran(0, "a\nb\n", ""), ran(0, "a\n", ""));
    assert_eq!(
        other.differs(&reference),
        Some(Difference::Output {
            stream: Stream::Stdout,
            line: 2,
            reference: Some("b".to_owned()),
            other: None,
        })
    );
}

#[test]
fn stderr_is_compared_when_stdout_is_the_same() {
    let (reference, other) = (ran(0, "a", "one"), ran(0, "a", "two"));
    assert!(matches!(
        other.differs(&reference),
        Some(Difference::Output { stream: Stream::Stderr, line: 1, .. })
    ));
}

#[test]
fn the_median_is_the_middle_run() {
    let mut r = ran(0, "", "");
    r.runs = [5, 1, 9].map(Duration::from_millis).to_vec();
    assert_eq!(r.median(), Duration::from_millis(5));
}

#[test]
fn a_missing_final_line_break_is_a_difference_of_its_own() {
    let (reference, other) = (ran(0, "a\n", ""), ran(0, "a", ""));
    assert_eq!(
        other.differs(&reference),
        Some(Difference::Terminator { stream: Stream::Stdout, reference: true })
    );
}
