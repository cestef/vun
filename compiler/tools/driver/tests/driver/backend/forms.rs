//! The primitives of `lang` a program calls at run time (`programs/forms.vun`) lower and run alike
//! through every backend; `primitives` checks that programs call all of them.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_items::targets;
use vun_sema::target::Choice;

use super::{program, status_across, status_on};
use crate::common::Fixture;

#[test]
fn primitives_the_tables_declare_lower_and_run() {
    assert_eq!(status_across("backend-forms", &program("forms.vun")), Some(0));
}

/// A million guaranteed tail calls, which overflow the stack unless each replaces its caller's
/// frame (spec 09, section 3).
#[test]
fn guaranteed_tail_calls_run_in_constant_stack() {
    assert_eq!(status_on("backend-tail", &program("tail.vun"), &[targets::linux_arm64::kernel]), Some(0));
}

/// A backend that cannot jump to a callee in place refuses the program, never calls instead.
#[test]
fn a_backend_without_tail_calls_refuses_them() {
    let fixture = Fixture::new("backend-tail-refused");
    let path = fixture.root.write("main.vun", program("tail.vun"));
    let mut session = fixture.session();
    session.choice = Choice::of(targets::wasi_wasm32::p1);
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_wasm::INFO.id);
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::BACKEND_LACKS]);
}
