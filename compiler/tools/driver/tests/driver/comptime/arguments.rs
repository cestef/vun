//! A `$` argument reading a local the body computes when it runs (spec 03, section 7): a function
//! run at compile time binds it, generated code has no value to bind.

use vun_diag::errors;

use crate::backend;
use crate::common::Fixture;

/// A function looping over a local and passing it to a `$` parameter, run by `$if` while `main`
/// is typed, and `$for` over a tuple's items of different types.
const AT_COMPILE_TIME: &str = "\
pick := ($n: u32) -> u32 { n };

has := ($want: u32) -> bool {
    for i in [1, 2, 3] {
        if pick(i) == want { return true; }
    }
    false
};

total := () -> u32 {
    mut n: u32 = 0;
    $for x in .{ 1, 2, 3 } { n += x; }
    n
};

start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    $if has(2) { if total() == 6 { .ok } else { .code(1) } } else { .code(2) }
};
";

#[test]
fn a_compile_time_call_binds_the_locals_it_passes_to_dollar_parameters() {
    assert_eq!(backend::status("comptime-run-time-arguments", AT_COMPILE_TIME), Some(0));
}

/// The same loop, run by the program.
const AT_RUN_TIME: &str = "\
pick := ($n: u32) -> u32 { n };

each := (x: u32) -> u32 {
    for i in [1, 2, 3] {
        if i == x { return pick(i); }
    }
    0
};

start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    if each(2) == 2 { .ok } else { .code(1) }
};
";

#[test]
fn generated_code_has_no_value_to_bind_to_a_dollar_parameter() {
    let fixture = Fixture::new("comptime-run-time-arguments-refused");
    let path = fixture.root.write("main.vun", AT_RUN_TIME);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let refused = session.lir(file).as_ref().as_ref().expect_err("`i` is known when running").clone();
    assert_eq!(refused.diagnostic().id, errors::BACKEND_UNLOWERED);
}
