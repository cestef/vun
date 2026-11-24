//! Loading a library as a program runs (`std.os.library`): a program loads one built beside it,
//! calls a function of it by its type and name, and unloads it.

use std::path::Path;

use vun_diag::errors;
use vun_driver::compare::Ran;
use vun_items::{Item, targets};

use crate::backend;
use crate::support::Fixture;

/// A library of one function, as a program of `os` holds the services its compiler assumes.
fn library(services: &str) -> String {
    format!(
        "{services}
#[link.export(\"calc_add\"), abi.c]
add := (a: i32, b: i32) -> i32 {{ (a + b).wrap(i32) }};
"
    )
}

/// A program loading the library at `PATH`, calling `calc_add(40, 2)`, and asking for a function
/// the library has none of, which adds nothing: it exits with 42, or with a code saying where it
/// failed.
const LOADER: &str = "\
start := std.os.start(main);

Add := #[abi.c] (a: i32, b: i32) -> i32;

main := (sys: &mut std.os.System) -> std.os.Exit {
    library := match sys.load(PATH) {
        .ok(l) => l,
        .err(_) => return .code(2),
    };
    add := match unsafe library.function(Add, \"calc_add\") {
        .ok(f) => f,
        .err(_) => return .code(3),
    };
    absent := match unsafe library.function(Add, \"calc_nothing\") {
        .ok(_) => return .code(4),
        .err(.unresolved) => 0,
        .err(_) => return .code(5),
    };
    sum := add(40, 2);
    match library.close() {
        .ok(_) => .of((sum + absent).wrap(Int(0..=125))),
        .err(_) => .code(6),
    }
};
";

/// The exit status of `LOADER` built for `target` through each backend, loading the library built
/// beside it from `services`, with `path` the text a program of the target gives the system for the
/// library's file; none where the machine does not run the target's programs.
fn loaded(name: &str, target: Item, services: &str, path: impl Fn(&Path) -> String) -> Option<i32> {
    let fixture = Fixture::new(name);
    let (mut session, calc) = backend::on(&fixture, "calc.vun", &library(services), target);
    let native =
        cfg!(all(target_os = "linux", target_arch = "x86_64")) && target == targets::linux_x86_64::gnu;
    if !native && session.runner().is_empty() {
        return None;
    }
    backend::sharing(&mut session);
    let built = session.build(calc, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let loader = LOADER.replace("PATH", &format!("{:?}", path(&built)));
    let (mut session, file) = backend::on(&fixture, "main.vun", &loader, target);
    let runs = session.run_each(file, 1).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let statuses: Vec<_> = runs.iter().map(|r| (r.backend, r.output.status.code())).collect();
    assert!(Ran::agree(&runs), "the backends disagree: {statuses:?}");
    statuses[0].1
}

#[test]
fn a_program_loads_a_library_and_calls_its_function() {
    // The GNU C library loads it, from the machine's own file system.
    let gnu = loaded("load-gnu", targets::linux_x86_64::gnu, "", |p| p.display().to_string());
    // Where the machine is such a system, the program runs.
    assert_eq!(gnu.is_some(), cfg!(all(target_os = "linux", target_arch = "x86_64")));
    assert!(gnu.is_none_or(|status| status == 42), "linux gnu: {gnu:?}");
    // A DLL has no C runtime to hold what its compiler assumes, so it holds the system's services;
    // Wine finds the file by the drive that maps the machine's root.
    for (name, target) in
        [("load-windows", targets::windows_x86_64::msvc), ("load-windows-x86", targets::windows_x86::msvc)]
    {
        let windows = loaded(name, target, "services := std.os.windows.services;\n", |p| {
            format!("Z:{}", p.display().to_string().replace('/', "\\"))
        });
        assert!(windows.is_none_or(|status| status == 42), "{name}: {windows:?}");
    }
}

/// A function made of an address that is the address of something else.
const NOT_A_FUNCTION: &str = "\
start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    _ = unsafe lang.mem.function_at(i32, 4096);
    .ok
};
";

#[test]
fn only_a_function_type_is_made_of_an_address() {
    let fixture = Fixture::new("load-not-a-function");
    let path = fixture.root.write("main.vun", NOT_A_FUNCTION);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let refused = session.lir(file).as_ref().as_ref().expect_err("an integer is no function").clone();
    let d = refused.diagnostic();
    assert_eq!(d.id, errors::BACKEND_UNLOWERED);
    assert!(d.message.to_string().contains("`i32` is no function type"), "{}", d.message);
}
