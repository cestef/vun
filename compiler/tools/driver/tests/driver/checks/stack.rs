//! The checks of the stack: each function's frame, against the bounds the runtime gives.

use vun_diag::{DiagId, errors};
use vun_intern::Symbol;
use vun_items::targets;
use vun_lir::Address;
use vun_sema::target::Choice;
use vun_target::Check;

use super::{Ran, TRAPPED, checked_run};
use crate::common::Fixture;

/// A program recursing past the stack, each call holding a kilobyte.
const DEEP: &str = "\
start := std.os.start(main);
down := (n: u32) -> u8 {
    mut pad: [1000]u8 = [0; 1000];
    pad[n.to(usize) % 1000] = 1;
    if n == 0 { pad[0] } else { (down(n - 1) + pad[1]).wrap(u8) }
};
main := (sys: &mut std.os.System) -> std.os.Exit {
    std.fmt.print(&mut sys.stdout(), \"{}\\n\", down(100000));
    .ok
};
";

#[test]
fn a_function_under_the_stack_checks_names_the_runtime_call_and_its_site() {
    let fixture = Fixture::new("checks-stack");
    let path = fixture.root.write("main.vun", DEEP);
    let lowered = |checks: &[Check]| {
        let mut session = fixture.session();
        session.choice = Choice::of(targets::wasi_wasm32::p1);
        session.add_checks(checks.iter().copied());
        let file = session.load(&path).unwrap();
        let unit = session.lir(file).as_ref().clone().unwrap();
        assert_eq!(unit.verify(session.origins), Ok(()));
        unit
    };
    let unchecked = lowered(&[]);
    assert!(unchecked.functions.iter().all(|(_, f)| f.stack_check.is_none()));
    let checked = lowered(&[Check::Stack]);
    let calls: Vec<_> = checked.functions.iter().filter_map(|(_, f)| f.stack_check).collect();
    assert!(!calls.is_empty());
    // The runtime of WASI exports the operation, so the program calls a function of its own.
    for check in calls {
        assert!((check.site as usize) < checked.sites.len() && matches!(check.call, Address::Func(_)));
    }
}

#[test]
fn a_backend_that_cannot_check_frames_refuses_the_stack_checks() {
    let fixture = Fixture::new("checks-stack-refused");
    let path = fixture.root.write("main.vun", DEEP);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::wasi_wasm32::p1);
    session.codegen.backend = Some(Symbol::intern("c"));
    session.add_checks([Check::Stack]);
    let file = session.load(&path).unwrap();
    let _ = session.compile(file);
    let ids: Vec<DiagId> = session.diag.sorted(session.origins).into_iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::BACKEND_LACKS]);
}

#[test]
fn a_wasi_program_running_past_its_stack_traps_under_the_stack_checks() {
    let target = targets::wasi_wasm32::p1;
    let run = |checks: &[Check]| checked_run("checks-stack-run", target, "wasm", checks, DEEP);
    let Ran::Exited(status) = run(&[Check::Stack]) else { return };
    assert_eq!(status, TRAPPED);
    // Unchecked, the engine's own fault ends it.
    let Ran::Exited(unchecked) = run(&[]) else { return };
    assert_ne!(unchecked, TRAPPED);
}
