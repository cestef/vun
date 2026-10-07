//! The primitives of `lang` a program calls at run time (`programs/forms.vun`) lower and run alike
//! through every backend; `primitives` checks that programs call all of them.

use vun_items::targets;

use super::{program, status, status_across, status_on};

#[test]
fn primitives_the_tables_declare_lower_and_run() {
    assert_eq!(status_across("backend-forms", &program("forms.vun")), Some(0));
}

/// A million guaranteed tail calls, which overflow the stack unless each replaces its caller's
/// frame (spec 09, section 3).
#[test]
fn guaranteed_tail_calls_run_in_constant_stack() {
    assert_eq!(
        status_on(
            "backend-tail",
            &program("tail.vun"),
            &[targets::linux_arm64::kernel, targets::linux_arm::kernel, targets::wasi_wasm32::p1]
        ),
        Some(0)
    );
}

/// A callee taking arguments on the stack: the native backends refuse the tail call, which C makes.
const STACK_ARGUMENTS: &str = "\
start := std.os.start(main);

wide := (a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, i: u32) -> u32 {
    (a + b + c + d + e + f + g + h + i).wrap(u32)
};

go := (n: u32) -> u32 { return lang.flow.tail(wide, (n, n, n, n, n, n, n, n, n)); };

main := (sys: &mut std.os.System) -> std.os.Exit { std.os.Exit.of(if go(1) == 9 { 0 } else { 1 }) };
";

/// A backend that cannot jump to a callee in place leaves the function to one that can.
#[test]
fn a_tail_call_that_needs_the_stack_runs_through_the_backend_that_can() {
    assert_eq!(status("backend-tail-stack", STACK_ARGUMENTS), Some(0));
}
