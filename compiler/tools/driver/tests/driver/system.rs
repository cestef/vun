//! What a program is linked against and takes from the system it runs on (spec 07, section 5, and
//! spec 08, section 2): the library an import names, the libraries a package names, the heap and
//! the environment of the C library's runtime.

use vun_items::targets;
use vun_sema::target::Choice;

use crate::common::Fixture;
use crate::variadic::status;

/// A program whose import comes from the library `m`, which is not linked unless the import names it.
const MATH: &str = "\
#[link.symbol(\"cbrt\"), link.library(\"m\"), abi.c]
cube_root: unsafe (x: f64) -> f64 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    root := unsafe { cube_root(27.0) };
    if root > 2.999 and root < 3.001 { .code(3) } else { .code(1) }
};
";

#[test]
fn the_library_an_import_names_is_linked() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    assert_eq!(status("link-library", MATH, targets::linux_x86_64::gnu), Some(3));
}

/// A program that exits with 3 when the variable `VUN_ENV_PROBE` is `probe`, taking its memory from
/// the heap of the runtime.
const ENVIRONMENT: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut heap := std.os.heap.Heap{};
    mut copy := std.text.string.String.empty();
    status := match sys.env(\"VUN_ENV_PROBE\") {
        .some(value) => if copy.push_str(std.os.heap.Heap, &mut heap, value).is_err() {
            2
        } else if copy.as_text() == \"probe\" {
            3
        } else {
            1
        },
        .none => 1,
    };
    copy.deinit(std.os.heap.Heap, &mut heap);
    .code(status)
};
";

#[test]
fn a_program_reads_the_environment_and_takes_memory_from_the_heap_of_the_c_library() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let fixture = Fixture::new("link-environment");
    let path = fixture.root.write("main.vun", ENVIRONMENT);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::linux_x86_64::gnu);
    let file = session.load(&path).unwrap();
    let built = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let with = std::process::Command::new(&built).env("VUN_ENV_PROBE", "probe").status().unwrap();
    assert_eq!(with.code(), Some(3));
    let without = std::process::Command::new(&built).env_remove("VUN_ENV_PROBE").status().unwrap();
    assert_eq!(without.code(), Some(1));
}

/// A package naming the library `m` for its programs, whose import names none.
const PACKAGE: &str = "package := .{
    .name = \"mathy\", .id = 77, .version = \"0.1.0\", .edition = 2026,
    .system = .{ \"m\" },
    .include = .{ \"build.vun\", \"main.vun\" },
};
";

const BARE_IMPORT: &str = "\
#[link.symbol(\"cbrt\"), abi.c]
cube_root: unsafe (x: f64) -> f64 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    root := unsafe { cube_root(27.0) };
    if root > 2.999 and root < 3.001 { .code(3) } else { .code(1) }
};
";

#[test]
fn a_package_names_the_system_libraries_its_programs_link() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let fixture = Fixture::new("link-system");
    fixture.root.write("build.vun", PACKAGE);
    let path = fixture.root.write("main.vun", BARE_IMPORT);
    let mut session = fixture.session();
    session.choice = Choice::of(targets::linux_x86_64::gnu);
    let file = session.load(&path).unwrap();
    let built = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(std::process::Command::new(built).status().unwrap().code(), Some(3));
}
