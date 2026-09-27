//! Linking (spec 13, section 7): a program linked by each linker that stands in for GNU ld runs
//! as it does linked by GNU ld; a linker is refused for a format it does not link.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_intern::Symbol;
use vun_sema::target::Choice;
use vun_target::{Name, Output};
use vun_tool::Executable;

use crate::common::Fixture;

/// The program each linker links: it exits with 3.
const EXITS: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .code(3) };
";

/// The linkers standing in for GNU ld, each with the program it runs, which the machine may lack.
const LINKERS: [(Name, &str); 2] = [(vun_link_mold::INFO.id, "mold"), (vun_link_wild::INFO.id, "wild")];

#[test]
fn a_program_runs_as_each_linker_links_it() {
    for (linker, program) in LINKERS {
        if Executable::search(program).is_none() {
            continue;
        }
        let fixture = Fixture::new(&format!("link-{linker}"));
        let path = fixture.root.write("main.vun", EXITS);
        let mut session = fixture.session();
        let file = session.load(&path).unwrap();
        session.codegen =
            Codegen { linker: Some(linker.symbol()), ..Codegen::through(vun_backend_native::INFO.id) };
        let built = session.build(file, None).unwrap_or_else(|_| panic!("{linker}: {:#?}", session.diag));
        let status = std::process::Command::new(built).status().unwrap();
        assert_eq!(status.code(), Some(3), "linked by {linker}");
    }
}

#[test]
fn a_linker_is_refused_for_a_format_it_does_not_link() {
    let fixture = Fixture::new("link-refused");
    let path = fixture.root.write("main.vun", EXITS);
    let mut session = fixture.session();
    session.choice = Choice::Path(["macos", "arm64"].map(Symbol::intern).to_vec());
    let file = session.load(&path).unwrap();
    session.codegen = Codegen {
        linker: Some(vun_link_mold::INFO.id.symbol()),
        ..Codegen::through(vun_backend_native::INFO.id)
    };
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::LINK_UNSUPPORTED]);
}

/// A library: its exports are its interface, and it has no runtime.
const LIBRARY: &str = "\
#[link.export(\"calc_add\"), abi.c]
add := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };
";

/// A C program linking the library `calc` and exiting with `calc_add(40, 2)`.
const USER: &str = "int calc_add(int, int);\nint main(void) { return calc_add(40, 2); }\n";

#[test]
fn a_shared_library_is_linked_by_a_c_program_through_each_backend() {
    for backend in [vun_backend_c::INFO.id, vun_backend_native::INFO.id] {
        let fixture = Fixture::new(&format!("link-shared-{backend}"));
        let path = fixture.root.write("calc.vun", LIBRARY);
        let mut session = fixture.session();
        let file = session.load(&path).unwrap();
        session.codegen = Codegen { output: Output::Shared, ..Codegen::through(backend) };
        let library = session.build(file, None).unwrap_or_else(|_| panic!("{backend}: {:#?}", session.diag));
        assert_eq!(library.file_name().and_then(|n| n.to_str()), Some("libcalc.so"));
        let dir = library.parent().unwrap();
        let user = fixture.root.write("user.c", USER);
        let program = fixture.root.join("user");
        let built = std::process::Command::new("cc")
            .arg(&user)
            .arg("-L")
            .arg(dir)
            .arg("-lcalc")
            .arg("-o")
            .arg(&program)
            .status()
            .unwrap();
        assert!(built.success(), "{backend}");
        let ran = std::process::Command::new(&program).env("LD_LIBRARY_PATH", dir).status().unwrap();
        assert_eq!(ran.code(), Some(42), "{backend}");
    }
}

#[test]
fn a_shared_library_on_wasi_is_a_module_of_its_exports() {
    let Some(wasmtime) = Executable::search("wasmtime") else { return };
    let fixture = Fixture::new("link-shared-wasi");
    let path = fixture.root.write("calc.vun", LIBRARY);
    let mut session = fixture.session();
    session.choice = Choice::Path(["wasi", "wasm32"].map(Symbol::intern).to_vec());
    let file = session.load(&path).unwrap();
    session.codegen = Codegen { output: Output::Shared, ..Codegen::through(vun_backend_wasm::INFO.id) };
    let module = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(module.file_name().and_then(|n| n.to_str()), Some("calc.wasm"));
    let ran = std::process::Command::new(wasmtime)
        .args(["--invoke", "calc_add"])
        .arg(&module)
        .args(["40", "2"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout).trim(), "42");
}
