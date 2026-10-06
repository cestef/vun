//! Linking (spec 13, section 7): a program linked by each linker that stands in for GNU ld runs
//! as it does linked by GNU ld; a linker is refused for a format it does not link.

use vun_diag::errors;
use vun_driver::Codegen;
use vun_items::targets;
use vun_sema::target::Choice;
use vun_target::{Name, Output};
use vun_tool::Executable;

use crate::backend;
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
    session.choice = Choice::of(targets::macos_arm64::darwin);
    let file = session.load(&path).unwrap();
    session.codegen = Codegen {
        linker: Some(vun_link_mold::INFO.id.symbol()),
        ..Codegen::through(vun_backend_native::INFO.id)
    };
    assert!(session.build(file, None).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).into_iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::LINK_UNSUPPORTED]);
}

/// A library: its exports are its interface, and it has no runtime.
const LIBRARY: &str = "\
#[link.export(\"calc_add\"), abi.c]
add := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };
";

/// A C program linking the library `calc` and exiting with `calc_add(40, 2)`.
const USER: &str = "int calc_add(int, int);\nint main(void) { return calc_add(40, 2); }\n";

/// The kinds of library each backend builds, each with the file it is.
const KINDS: [(Output, &str); 2] = [(Output::Shared, "so"), (Output::Static, "a")];

/// The exit status of the C program `user`, linked against the library `name` built from
/// `source` as `output` through `backend`, and the threads library.
pub(crate) fn run_with_library(
    name: &str,
    source: &str,
    user: &str,
    output: Output,
    backend: Name,
) -> Option<i32> {
    let fixture = Fixture::new(&format!("link-{name}-{}-{backend}", output.name()));
    let path = fixture.root.write(format!("{name}.vun"), source);
    let mut session = fixture.session();
    let loaded = session.load(&path).unwrap();
    session.codegen = Codegen { output, ..Codegen::through(backend) };
    let library = session.build(loaded, None).unwrap_or_else(|_| panic!("{backend}: {:#?}", session.diag));
    let extension = KINDS.iter().find(|(kind, _)| *kind == output).map(|(_, e)| *e);
    assert_eq!(library.extension().and_then(|e| e.to_str()), extension, "{backend} {output:?}");
    let dir = library.parent().unwrap();
    let user = fixture.root.write("user.c", user);
    let program = fixture.root.join("user");
    let built = std::process::Command::new("cc")
        .arg(&user)
        .arg("-L")
        .arg(dir)
        .arg(format!("-l{name}"))
        .arg("-lpthread")
        .arg("-o")
        .arg(&program)
        .status()
        .unwrap();
    assert!(built.success(), "{backend} {output:?}");
    std::process::Command::new(&program).env("LD_LIBRARY_PATH", dir).status().unwrap().code()
}

#[test]
fn a_library_is_linked_by_a_c_program_through_each_backend() {
    for backend in [vun_backend_c::INFO.id, vun_backend_native::INFO.id] {
        for (output, _) in KINDS {
            assert_eq!(
                run_with_library("calc", LIBRARY, USER, output, backend),
                Some(42),
                "{backend} {output:?}"
            );
        }
    }
}

/// A library counting in storage per thread (spec 07, section 2).
const COUNTED: &str = "\
count: std.sync.Local(i32) = .init(0);

#[link.export(\"counted_bump\"), abi.c]
bump := () -> i32 {
    count.set((count.get() + 1).wrap(i32));
    count.get()
};
";

/// A C program counting once in its main thread and three times in another, exiting with each
/// thread's last count: 2 and 3, as neither sees the other's.
const THREADED_USER: &str = "#include <pthread.h>
int counted_bump(void);
static void *run(void *arg) { counted_bump(); counted_bump(); return (void *)(long)counted_bump(); }
int main(void) {
    counted_bump();
    pthread_t t;
    void *theirs;
    pthread_create(&t, 0, run, 0);
    pthread_join(t, &theirs);
    return counted_bump() * 10 + (int)(long)theirs;
}
";

#[test]
fn a_library_keeps_storage_per_thread_through_each_backend() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    for backend in [vun_backend_c::INFO.id, vun_backend_native::INFO.id] {
        for (output, _) in KINDS {
            let status = run_with_library("counted", COUNTED, THREADED_USER, output, backend);
            assert_eq!(status, Some(23), "{backend} {output:?}");
        }
    }
}

#[test]
fn a_shared_library_on_wasi_is_a_module_of_its_exports() {
    let Some(wasmtime) = Executable::search("wasmtime") else { return };
    let fixture = Fixture::new("link-shared-wasi");
    let path = fixture.root.write("calc.vun", LIBRARY);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::wasi_wasm32::p1);
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

/// A library for Windows, which holds the services its compiler assumes (a library has no
/// runtime to hold them): two exports, one of them with floating point.
const WINDOWS_LIBRARY: &str = "\
services := std.os.windows.services;

#[link.export(\"calc_add\"), abi.c]
add := (a: i32, b: i32) -> i32 { (a + b).wrap(i32) };

#[link.export(\"calc_scale\"), abi.c]
scale := (x: f64, by: f64) -> f64 { x * by };
";

#[test]
fn a_windows_library_is_a_dll_exporting_its_interface() {
    let fixture = Fixture::new("link-windows-dll");
    let (mut session, file) =
        backend::on(&fixture, "calc.vun", WINDOWS_LIBRARY, targets::windows_x86_64::msvc);
    // Where the machine keeps no import libraries, there is nothing to link against.
    let sysroot = session.target.clone().and_then(Result::ok).and_then(|spec| spec.c.compiler.sysroot);
    if sysroot.is_none() {
        return;
    }
    backend::sharing(&mut session);
    let built = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(built.file_name().and_then(|n| n.to_str()), Some("calc.dll"));
    let image = vun_pe::Image::read(&std::fs::read(built).unwrap()).unwrap();
    assert!(image.library);
    assert_eq!(image.entry, 0, "nothing starts a library");
    let exports: Vec<_> = image.exports.iter().filter_map(|e| e.name.as_deref()).collect();
    for name in [&b"calc_add"[..], b"calc_scale", b"__chkstk", b"memcpy"] {
        assert!(exports.contains(&name), "{} is exported: {exports:?}", String::from_utf8_lossy(name));
    }
}

/// What a Mach-O file's header starts with: its magic, 64-bit; its processor, arm64; and its
/// kind, an executable.
const MACHO_EXECUTABLE: [u32; 3] = [0xfeed_facf, 0x0100_000c, 2];

#[test]
fn a_macos_program_links_against_the_sdk_the_toolchain_names_through_each_backend() {
    for backend in [vun_backend_native::INFO.id, vun_backend_c::INFO.id] {
        let fixture = Fixture::new(&format!("link-macos-{backend}"));
        let path = fixture.root.write("main.vun", EXITS);
        let mut session = fixture.session();
        session.choice = Choice::of(targets::macos_arm64::darwin);
        let file = session.load(&path).unwrap();
        let _ = session.lir(file);
        // Where the machine keeps no SDK, there is nothing to link against.
        let sdk = session.target.clone().and_then(Result::ok).and_then(|spec| spec.c.compiler.sysroot);
        if sdk.is_none() {
            return;
        }
        session.codegen = Codegen::through(backend);
        let built = session.build(file, None).unwrap_or_else(|_| panic!("{backend}: {:#?}", session.diag));
        let bytes = std::fs::read(built).unwrap();
        let header: Vec<u32> =
            bytes.as_chunks::<4>().0.iter().take(4).map(|w| u32::from_le_bytes(*w)).collect();
        assert_eq!([header[0], header[1], header[3]], MACHO_EXECUTABLE, "{backend}");
    }
}
