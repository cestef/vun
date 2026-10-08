//! The primitives of `lang` a program calls at run time (`programs/forms.vun`) lower and run alike
//! through every backend; `primitives` checks that programs call all of them.

use vun_driver::Codegen;
use vun_items::targets;

use super::{program, status_across, status_on, status_where};
use crate::common::Fixture;

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

/// Arguments on the stack, records and results in memory: a tail call writes them where its
/// caller received its own.
#[test]
fn guaranteed_tail_calls_pass_values_in_memory_in_constant_stack() {
    assert_eq!(
        status_on(
            "backend-tail-memory",
            &program("tail_memory.vun"),
            &[targets::linux_arm64::kernel, targets::linux_arm::kernel, targets::wasi_wasm32::p1]
        ),
        Some(0)
    );
}

/// A result the callee writes through the caller's address, which the C compilers cannot jump
/// with: GCC refuses a `musttail` call returning a struct, so the C backend refuses it too.
#[test]
fn guaranteed_tail_calls_write_aggregate_results_through_the_callers_address() {
    assert_eq!(
        status_where(
            "backend-tail-aggregate",
            &program("tail_aggregate.vun"),
            &[targets::linux_arm64::kernel, targets::linux_arm::kernel, targets::wasi_wasm32::p1],
            |backend| backend.info().id != vun_backend_c::INFO.id
        ),
        Some(0)
    );
}

/// C records too large for registers, which arm64 passes by the address of a copy: the call passes
/// on the copy it received. The C compilers cannot jump with such an argument, so the C backend
/// refuses it.
#[test]
fn guaranteed_tail_calls_pass_large_c_records_in_constant_stack() {
    assert_eq!(
        status_where(
            "backend-tail-record",
            &program("tail_record.vun"),
            &[targets::linux_arm64::kernel, targets::linux_arm::kernel, targets::wasi_wasm32::p1],
            |backend| backend.info().id != vun_backend_c::INFO.id
        ),
        Some(0)
    );
}

#[test]
fn the_native_backend_lists_its_code_under_the_source_lines() {
    let fixture = Fixture::new("backend-native-listing");
    let path = fixture.root.write("main.vun", program("loops.vun"));
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_native::INFO.id);
    let mut out = Vec::new();
    session.dump_code(file, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    // A frame's prologue, as the host's instruction set writes it.
    let prologue = if cfg!(target_arch = "aarch64") { "stp x29, x30" } else { "push rbp" };
    assert!(text.lines().any(|l| l.contains(prologue)), "{text}");
    assert!(text.lines().any(|l| l.starts_with("  ; ") && l.contains("main.vun:")), "{text}");
}
