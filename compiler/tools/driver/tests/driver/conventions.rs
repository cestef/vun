//! Calling conventions a function names (spec 07, section 4): `#[abi.win64]` and `#[abi.sysv]`
//! beside the target's own on x86-64, called by C compiled under each, and refused where the
//! target's architecture has none of that name or the backend writes no code for it.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_items::targets;
use vun_link::{LinkPlan, Linker};
use vun_link_lld::LldLinker;
use vun_sema::target::Choice;
use vun_target::Output;
use vun_tool::Executable;

use crate::backend;
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
    session.choice = Choice::of(targets::linux_x86_64::kernel);
    let file = session.load(&path).unwrap();
    session.codegen = Codegen { output: Output::Shared, ..Codegen::through(vun_backend_native::INFO.id) };
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::BACKEND_LACKS]);
}

/// A Windows library of one function under each convention of 32-bit x86, which x86-64 takes for
/// its one: it holds the services the system's C runtime would.
const WINDOWS_LIBRARY: &str = "\
services := std.os.windows.services;

#[link.export(\"conv_c\"), abi.c]
plain := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };

#[link.export(\"conv_std\"), abi.stdcall]
std_call := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };

#[link.export(\"conv_fast\"), abi.fastcall]
fast := (a: i32, b: i32, c: i32) -> i32 { (a + b + c).wrap(i32) };

#[link.export(\"conv_this\"), abi.thiscall]
method := (me: *i32, b: i32) -> i32 { (unsafe me.read() + b).wrap(i32) };
";

/// C for Windows loading the library and calling each function as its convention says: `3 + 7 +
/// 6 + 105`.
const WINDOWS_USER: &str = "\
typedef void *(__stdcall *proc_t)(void);
__declspec(dllimport) void *__stdcall LoadLibraryA(const char *);
__declspec(dllimport) proc_t __stdcall GetProcAddress(void *, const char *);
typedef int (__cdecl *c_t)(int, int);
typedef int (__stdcall *s_t)(int, int);
typedef int (__fastcall *f_t)(int, int, int);
typedef int (__thiscall *t_t)(int *, int);
int _fltused;
int mainCRTStartup(void) {
    void *m = LoadLibraryA(\"conv.dll\");
    if (!m) return 1;
    c_t c = (c_t)GetProcAddress(m, \"conv_c\");
    s_t s = (s_t)GetProcAddress(m, \"conv_std\");
    f_t f = (f_t)GetProcAddress(m, \"conv_fast\");
    t_t t = (t_t)GetProcAddress(m, \"conv_this\");
    if (!c || !s || !f || !t) return 2;
    int me = 100;
    return c(1, 2) + s(3, 4) + f(1, 2, 3) + t(&me, 5);
}
";

#[test]
fn a_windows_library_is_called_by_c_under_each_convention_of_its_architecture() {
    if Executable::search("clang").is_none() || Executable::search("lld-link").is_none() {
        return;
    }
    for (name, target) in
        [("conv-x86", targets::windows_x86::msvc), ("conv-x86-64", targets::windows_x86_64::msvc)]
    {
        let fixture = Fixture::new(name);
        let (mut session, file) = backend::on(&fixture, "conv.vun", WINDOWS_LIBRARY, target);
        let Some(spec) = session.target.clone().and_then(Result::ok) else { continue };
        if spec.c.compiler.sysroot.is_none() || session.runner().is_empty() {
            continue;
        }
        backend::sharing(&mut session);
        let library = session.build(file, None).unwrap_or_else(|_| panic!("{name}: {:#?}", session.diag));
        // The program is beside the library, where the system looks for it first. It is compiled as
        // the target's C is, and linked as its programs are.
        let dir = library.parent().unwrap();
        let source = fixture.root.write("user.c", WINDOWS_USER);
        let object = dir.join("user.o");
        let c = &spec.c.compiler;
        let compiled = std::process::Command::new(c.command.as_str())
            .args(c.flags.iter().map(|flag| flag.as_str()))
            .arg("-c")
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap();
        assert!(compiled.success(), "{name}");
        let program = dir.join("user.exe");
        let plan = LinkPlan {
            inputs: vec![object],
            output: program.clone(),
            kind: Output::Executable,
            entry: spec.entry_symbol,
            exports: Vec::new(),
            regions: Vec::new(),
            sections: Vec::new(),
        };
        LldLinker
            .link(&plan, &spec, &mut session.diag)
            .unwrap_or_else(|_| panic!("{name}: {:#?}", session.diag));
        let status = session.command(&program).current_dir(dir).status().unwrap();
        assert_eq!(status.code(), Some(121), "{name}");
    }
}
