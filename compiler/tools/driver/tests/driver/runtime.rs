//! The Linux runtime calling the kernel (spec 08, section 2): the default Linux targets build
//! static programs, without a C library, that start at the runtime's `_start`.

use vun_harness::{ACCEPT, repo_root};

use crate::backend::{BACKENDS, build, status, status_on};

/// The types of an ELF program's segments: its program headers' `p_type`, read from the
/// 64-bit little-endian layout (`e_phoff` at 32, `e_phentsize` at 54, `e_phnum` at 56).
fn segments(elf: &[u8]) -> Vec<u32> {
    let int =
        |at: usize, n: usize| elf[at..at + n].iter().rev().fold(0usize, |v, &b| v << 8 | usize::from(b));
    let (offset, size, count) = (int(32, 8), int(54, 2), int(56, 2));
    (0..count).map(|i| u32::try_from(int(offset + i * size, 4)).unwrap()).collect()
}

/// `PT_DYNAMIC` and `PT_INTERP`: what a program the dynamic linker starts has.
const DYNAMIC: [u32; 2] = [2, 3];

#[test]
fn a_linux_program_is_static_and_starts_at_the_runtime() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    for backend in BACKENDS {
        let (_fixture, program) = build("runtime-static", &text, backend);
        let segments = segments(&std::fs::read(&program).unwrap());
        assert!(
            !segments.is_empty() && !segments.iter().any(|s| DYNAMIC.contains(s)),
            "{backend}: {segments:?}"
        );
        let ran = std::process::Command::new(&program).output().unwrap();
        assert_eq!((ran.status.code(), ran.stdout.as_slice()), (Some(0), &b"hello\n"[..]), "{backend}");
    }
}

/// A program moving bytes over themselves both ways, filling, copying and comparing through
/// the runtime's functions of C's `string.h`: `[2, 3, 4, 7, 2, 4, 9, 9]`, `2 + 7 + 2 + 9`, and
/// 100 where the comparisons hold.
const MEMORY: &str = "\
#[link.symbol(\"memmove\"), abi.c]
move: (to: *u8, from: *u8, bytes: usize) -> *u8 = _;
#[link.symbol(\"memcpy\"), abi.c]
copy: (to: *u8, from: *u8, bytes: usize) -> *u8 = _;
#[link.symbol(\"memset\"), abi.c]
fill: (to: *u8, value: i32, bytes: usize) -> *u8 = _;
#[link.symbol(\"memcmp\"), abi.c]
compare: (a: *u8, b: *u8, bytes: usize) -> i32 = _;

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut b: [8]u8 = [1, 2, 3, 4, 5, 6, 7, 8];
    p := unsafe lang.mem.pointer_to([8]u8, &b).cast(u8);
    unsafe {
        move(p.offset(2), p, 4);
        move(p, p.offset(3), 4);
        fill(p.offset(6), 9, 2);
        copy(p.offset(4), p, 1);
    }
    less := compare(p, unsafe p.offset(1), 2) < 0 and compare(p, p, 8) == 0;
    std.os.Exit.of((b[0] + b[3] + b[4] + b[7] + if less { 100 } else { 0 }).to(Int(0..=125)))
};
";

#[test]
fn the_runtime_moves_fills_and_compares_memory_as_c_does() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(status_on("runtime-memory", MEMORY, &[&["linux", "arm64"]]), Some(120));
}

/// A corpus program of `programs/`, by its name.
fn corpus(name: &str) -> String {
    std::fs::read_to_string(repo_root().join(ACCEPT).join("programs").join(name)).unwrap()
}

#[test]
fn clocks_are_read_and_a_pause_lasts_on_every_runtime() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(status("runtime-clocks", &corpus("clocks.vun")), Some(0));
}

#[test]
fn a_file_written_reads_back_and_a_missing_one_is_missing() {
    if !cfg!(target_os = "linux") {
        return;
    }
    // The file goes in a directory of the test's own, which WASI's runner is not given.
    let dir = vun_fs::Scratch::create("runtime-files").unwrap();
    let written = dir.path().join("written");
    let source =
        corpus("files.vun").replace("\"files.vun.out\"", &format!("{:?}", written.display().to_string()));
    assert_eq!(status_on("runtime-files", &source, &[&["linux", "arm64"]]), Some(0));
}
