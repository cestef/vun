//! The wasm linker (spec 13, section 8.4), on the backend's objects (`objects`), on clang's
//! (`clang`) and on whole programs: it links what `wasm-ld` links into
//! programs that run alike, and links the wasm backend's objects with clang's, whose constructors,
//! sections, weak symbols and variadic functions work as C says.

use std::path::{Path, PathBuf};
use std::process::Command;

use vun_driver::Codegen;
use vun_harness::repo_root;
use vun_items::targets;
use vun_sema::target::Choice;
use vun_tool::Executable;

use crate::common::Fixture;

mod asm;
mod clang;
mod objects;

/// The programs of the corpus both linkers link, run with no arguments.
const PROGRAMS: [&str; 3] = ["hello.vun", "tour.vun", "locals.vun"];

/// What a program run gives: its status and what it printed.
fn run(wasmtime: &Path, program: &Path) -> (Option<i32>, String) {
    let ran = Command::new(wasmtime).arg("run").arg(program).output().unwrap();
    (ran.status.code(), String::from_utf8_lossy(&ran.stdout).into_owned())
}

/// Builds the corpus program `name` for WASI through the wasm backend, linked by `linker` or by
/// the target's.
fn build(name: &str, linker: Option<vun_target::Name>) -> (Fixture, PathBuf) {
    let source = std::fs::read_to_string(repo_root().join("corpus/accept/programs").join(name)).unwrap();
    build_source(name, &source, linker)
}

/// Builds the program `source`, called `name`, for WASI through the wasm backend.
fn build_source(name: &str, source: &str, linker: Option<vun_target::Name>) -> (Fixture, PathBuf) {
    let fixture = Fixture::new(&format!("wasm-{name}-{}", linker.map_or("own", |l| l.0)));
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::wasi_wasm32::p1);
    let file = session.load(&path).unwrap();
    let through = Codegen::through(vun_backend_wasm::INFO.id);
    session.codegen = Codegen { linker: linker.map(vun_target::Name::symbol), ..through };
    let program = session.build(file, None).unwrap_or_else(|_| panic!("{name}: {:#?}", session.diag));
    (fixture, program)
}

#[test]
fn programs_the_wasm_linker_links_run_as_those_wasm_ld_links() {
    let (Some(wasmtime), Some(_)) = (Executable::search("wasmtime"), Executable::search("wasm-ld")) else {
        return;
    };
    for name in PROGRAMS {
        let (_own, linked) = build(name, None);
        let (_lld, by_lld) = build(name, Some(vun_link_lld::INFO.id));
        assert_eq!(run(&wasmtime, &linked), run(&wasmtime, &by_lld), "{name}");
    }
}

/// C for WASI with no C library: a variadic function, a constructor, a section read through its
/// bounds and a weak function no object defines.
const C: &str = r#"
#include <stdarg.h>

long long vsum(int n, ...) {
    va_list ap;
    va_start(ap, n);
    long long t = va_arg(ap, int);
    t += (long long) va_arg(ap, double);
    t += va_arg(ap, long long);
    t += va_arg(ap, int);
    va_end(ap);
    return t;
}

static int ready;
__attribute__((constructor)) static void init(void) { ready = 40; }
int readiness(void) { return ready; }

__attribute__((section("registry"), used)) static const int items[] = { 1, 2, 3 };
extern const int __start_registry[], __stop_registry[];
int registered(void) {
    int t = 0;
    for (const int *p = __start_registry; p < __stop_registry; p++) t += *p;
    return t;
}

__attribute__((weak)) int missing(void);
int probe(void) { return missing ? 100 : 0; }
"#;

/// The Vun program calling it: `10 + 40 + 6 + 0`, once the constructors ran.
const CALLS: &str = r#"
#[link.symbol("vsum"), abi.c]
vsum: unsafe (n: i32, ..args: .{ i32, f64, i64, u8 }) -> i64 = _;
#[link.symbol("__wasm_call_ctors"), abi.c]
ctors: unsafe () -> () = _;
#[link.symbol("readiness"), abi.c]
readiness: unsafe () -> i32 = _;
#[link.symbol("registered"), abi.c]
registered: unsafe () -> i32 = _;
#[link.symbol("probe"), abi.c]
probe: unsafe () -> i32 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    unsafe ctors();
    total := unsafe vsum(4, 1, 2.5, 3, 4) + unsafe readiness() + unsafe registered() + unsafe probe();
    std.os.Exit.of(total.to(Int(0..=125)))
};
"#;

#[test]
fn objects_of_the_wasm_backend_and_of_clang_link_and_run_as_c_says() {
    let (Some(wasmtime), Some(clang)) = (Executable::search("wasmtime"), Executable::search("clang")) else {
        return;
    };
    let fixture = Fixture::new("wasm-with-c");
    let path = fixture.root.write("main.vun", CALLS);
    let c = fixture.root.write("c.c", C);
    let c_object = fixture.root.join("c.o");
    let compiled = Command::new(clang)
        .args(["--target=wasm32-wasip1", "-O1", "-c"])
        .arg(&c)
        .arg("-o")
        .arg(&c_object)
        .status();
    assert!(compiled.unwrap().success(), "clang compiles the C");
    let mut session = fixture.session();
    session.choice = Choice::of(targets::wasi_wasm32::p1);
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_wasm::INFO.id);
    let compiled = session.compile(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let objects: Vec<Vec<u8>> =
        compiled.objects.iter().chain([&c_object]).map(|path| std::fs::read(path).unwrap()).collect();
    let program = vun_link_wasm::WasmLinker::program(&objects, Some("_start")).unwrap();
    let linked = fixture.root.write("main.wasm", program);
    let own = run(&wasmtime, &linked).0;
    // `wasm-ld` links the same objects into a program that exits alike.
    let Some(wasm_ld) = Executable::search("wasm-ld") else {
        assert_eq!(own, Some(56));
        return;
    };
    let by_lld = fixture.root.join("lld.wasm");
    let status = Command::new(wasm_ld)
        .args(["--entry", "_start", "-o"])
        .arg(&by_lld)
        .args(compiled.objects.iter().chain([&c_object]))
        .status();
    assert!(status.unwrap().success(), "wasm-ld links the objects");
    assert_eq!((own, run(&wasmtime, &by_lld).0), (Some(56), Some(56)));
}
