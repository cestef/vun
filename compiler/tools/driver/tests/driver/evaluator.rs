//! The evaluator's limits (spec 03, section 7): calls nest as deep as their budget lets them,
//! on stack the evaluator adds, never overflowing the thread's.

use crate::support::Fixture;
use vun_diag::errors;

#[test]
fn compile_time_calls_nest_past_the_host_stack() {
    let fixture = Fixture::new("comptime-deep");
    // Twelve thousand calls deep overflow a thread's own 8 MiB, not the evaluator's stack.
    let file = fixture.root.write(
        "deep.vun",
        "\
count := (n: u32) -> u32 { if n == 0 { 0 } else { (count(n - 1) + 1).wrap(u32) } };
Deep: type = Int(0..=count(12000));
fits: Deep = 12000;
over: Deep = 12001;
",
    );
    let reported = fixture.reported(&file);
    assert_eq!(reported, [(errors::INT_DOES_NOT_FIT, "value does not fit the type `Deep`".to_owned())]);
}
