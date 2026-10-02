//! Calling conventions a function names (spec 07, section 4): `#[abi.win64]` and `#[abi.sysv]`
//! beside the target's own on x86-64, called by C compiled under each, and refused where the
//! target's architecture has none of that name or the backend writes no code for it.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_items::targets;
use vun_sema::target::Choice;
use vun_target::Output;

use crate::common::Fixture;
use crate::link::run_with_library;

/// Functions each under the other convention of x86-64 than the first the target's, taking more
/// arguments than registers carry in either, so the stack has to agree too.
const LIBRARY: &str = "\
#[link.export(\"conv_win\"), abi.win64]
win := (a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 { (a + b + c + d + e).wrap(i32) };

#[link.export(\"conv_sysv\"), abi.sysv]
sysv := (a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) -> i32 {
    (a + b + c + d + e + f + g).wrap(i32)
};
";

/// C calling each under the convention it is written for: `15 + 28`.
const USER: &str = "\
__attribute__((ms_abi)) int conv_win(int, int, int, int, int);
__attribute__((sysv_abi)) int conv_sysv(int, int, int, int, int, int, int);
int main(void) { return conv_win(1, 2, 3, 4, 5) + conv_sysv(1, 2, 3, 4, 5, 6, 7); }
";

#[test]
fn a_function_is_called_by_c_under_the_convention_it_names() {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return;
    }
    for output in [Output::Shared, Output::Static] {
        let status = run_with_library("conv", LIBRARY, USER, output, vun_backend_c::INFO.id);
        assert_eq!(status, Some(43), "{output:?}");
    }
}

/// A function of 32-bit x86's convention on a 64-bit target.
const STDCALL: &str = "\
start := std.os.start(main);

#[abi.stdcall]
add := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };

main := (sys: &mut std.os.System) -> std.os.Exit { std.os.Exit.of(add(1, 2).to(Int(0..=125))) };
";

#[test]
fn a_convention_of_another_architecture_is_refused() {
    let fixture = Fixture::new("conv-stdcall");
    let path = fixture.root.write("main.vun", STDCALL);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::linux_x86_64::kernel);
    let file = session.load(&path).unwrap();
    let refused = session.lir(file).as_ref().as_ref().expect_err("x86-64 has no stdcall").clone();
    assert_eq!(refused.diagnostic().id, errors::BACKEND_UNLOWERED);
}

#[test]
fn a_backend_without_conventions_refuses_them() {
    let fixture = Fixture::new("conv-native");
    let path = fixture.root.write("main.vun", LIBRARY);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen { output: Output::Shared, ..Codegen::through(vun_backend_native::INFO.id) };
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::BACKEND_LACKS]);
}
