//! The checks of unsafe code in a WASI program, whose shadow lives in linear memory the program
//! grows the first time a check needs it.

use vun_items::targets;
use vun_target::Check;

use super::{PAST, Ran, THIRD, TRAPPED, checked_run};

#[test]
fn a_wasi_program_runs_the_pointer_checks_over_a_shadow_in_grown_memory() {
    let run = |name: &str, source: &str| {
        checked_run(name, targets::wasi_wasm32::p1, "wasm", &[Check::Pointers], source)
    };
    let (Ran::Exited(within), Ran::Exited(past)) =
        (run("checks-wasi-third", THIRD), run("checks-wasi-past", PAST))
    else {
        return;
    };
    assert_eq!((within, past), (Some(10), TRAPPED));
}
