//! The Linux runtime calling the kernel (spec 08, section 2): the default Linux targets build
//! static programs, without a C library, that start at the runtime's `_start`.

use vun_harness::{ACCEPT, repo_root};
use vun_items::{Item, targets};

use crate::backend::{self, BACKENDS, build, status_across, status_on};
use crate::common::Fixture;

/// The types of an ELF program's segments: its program headers' `p_type`, read from the
/// 64-bit little-endian layout (`e_phoff` at 32, `e_phentsize` at 54, `e_phnum` at 56).
fn segments(elf: &[u8]) -> Vec<u32> {
    let int =
        |at: usize, n: usize| elf[at..at + n].iter().rev().fold(0usize, |v, &b| v << 8 | usize::from(b));
    let (offset, size, count) = (int(32, 8), int(54, 2), int(56, 2));
    (0..count).map(|i| u32::try_from(int(offset + i * size, 4)).unwrap()).collect()
}

/// The first line a trap reading past an array writes: its site's message.
const PAST: &str = "trap: an index past the last item";

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
    assert_eq!(status_on("runtime-memory", MEMORY, &[targets::linux_arm64::kernel]), Some(120));
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
    assert_eq!(status_across("runtime-clocks", &corpus("clocks.vun")), Some(0));
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
    assert_eq!(status_on("runtime-files", &source, &[targets::linux_arm64::kernel]), Some(0));
}

#[test]
fn each_thread_counts_in_its_own_storage_on_every_runtime() {
    if !cfg!(target_os = "linux") {
        return;
    }
    let cross: [Item; 3] =
        [targets::linux_arm64::kernel, targets::linux_arm::kernel, targets::wasi_wasm32::p1];
    assert_eq!(status_on("runtime-locals", &corpus("locals.vun"), &cross), Some(36));
}

#[test]
fn threads_run_beside_the_main_one_and_are_joined_on_every_runtime() {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert_eq!(status_across("runtime-threads", &corpus("threads.vun")), Some(36));
}

/// A trap three calls deep, each a call of the function itself, which is not inlined.
const DEEP: &str = "start := std.os.start(main);

dig := (items: &[8]u8, depth: usize, at: usize) -> u8 {
    if depth == 0 { items[at] } else { dig(items, (depth - 1).wrap(usize), at) }
};

main := (sys: &mut std.os.System) -> std.os.Exit {
    items: [8]u8 = [1, 2, 3, 4, 5, 6, 7, 8];
    if dig(&items, 2, (sys.args().len() + 8).wrap(usize)) > 0 { .ok } else { .code(1) }
};
";

#[test]
fn a_trap_writes_where_it_is_written_and_the_calls_that_led_to_it() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for backend in BACKENDS {
        let (fixture, program) = build("runtime-backtrace", DEEP, backend);
        let file = fixture.root.path().join("main.vun").display().to_string();
        let ran = std::process::Command::new(&program).output().unwrap();
        let text = String::from_utf8(ran.stderr).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let dig = format!("  in dig ({file}:3:8)");
        assert_eq!(ran.status.code(), Some(101), "{backend}");
        assert_eq!(lines[..4], [PAST, &format!("  at {file}:4:21"), &dig, &dig], "{backend}: {text}");
        assert!(lines[4..].iter().all(|l| l.starts_with("  in ")), "{backend}: {text}");
    }
}

/// A trap in a function inlined into one inlined in turn, as small callees are.
const INLINED: &str = "start := std.os.start(main);

pick := (items: &[8]u8, at: usize) -> u8 { items[at] };

sum := (items: &[8]u8, n: usize) -> u32 {
    mut total: u32 = 0;
    for i in 0..n { total += pick(items, i).to(u32); }
    total
};

main := (sys: &mut std.os.System) -> std.os.Exit {
    items: [8]u8 = [1, 2, 3, 4, 5, 6, 7, 8];
    if sum(&items, (sys.args().len() + 8).wrap(usize)) > 0 { .ok } else { .code(1) }
};
";

#[test]
fn a_trap_in_inlined_code_names_each_function_it_is_inlined_into() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for backend in BACKENDS {
        let (fixture, program) = build("runtime-inlined", INLINED, backend);
        let file = fixture.root.path().join("main.vun").display().to_string();
        let ran = std::process::Command::new(&program).output().unwrap();
        let text = String::from_utf8(ran.stderr).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let at = format!("  at {file}:3:44");
        let inlined = [PAST, &at, "  in pick, inlined", "  in sum, inlined", "  in main, inlined"];
        assert_eq!(lines[..5], inlined, "{backend}: {text}");
    }
}

/// A trap in a closure a thread runs, which ends that thread alone: joining it gives the trap.
const THREADED: &str = "start := std.os.start(main);

pick := (items: &[8]u8, at: usize) -> u8 { items[at] };

main := (sys: &mut std.os.System) -> std.os.Exit {
    items: [8]u8 = [1, 2, 3, 4, 5, 6, 7, 8];
    n := (sys.args().len() + 8).wrap(usize);
    mut f := [&items, n] () -> u32 { pick(&items, n).to(u32) };
    match sys.spawn(&mut f) {
        .ok(t) => match t.join() { .ok(_) => .code(1), .err(_) => .code(3) },
        .err(_) => .code(2),
    }
};
";

#[test]
fn a_trap_in_a_thread_ends_it_alone_naming_the_calls_up_to_where_it_starts_then_its_spawner() {
    if !cfg!(target_os = "linux") {
        return;
    }
    for backend in BACKENDS {
        let (fixture, program) = build("runtime-threaded", THREADED, backend);
        let file = fixture.root.path().join("main.vun").display().to_string();
        let ran = std::process::Command::new(&program).output().unwrap();
        let text = String::from_utf8(ran.stderr).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let at = format!("  at {file}:3:44");
        assert_eq!(ran.status.code(), Some(3), "{backend}: {text}");
        assert_eq!(
            lines[..4],
            [PAST, &at, "  in pick, inlined", "  in main.f, inlined"],
            "{backend}: {text}"
        );
        let spawned = lines.iter().position(|&l| l == "  spawned by").expect(&text);
        assert!(lines[spawned - 1].starts_with("  in Thread.run ("), "{backend}: {text}");
        let main = format!("  in main ({file}:");
        assert!(lines[spawned..].iter().any(|l| l.starts_with(&main)), "{backend}: {text}");
    }
}

/// A program that exits with 7 when it is started with the program's name and 601 arguments of more
/// than ten thousand bytes between them, the last being `last`: more than the 256 arguments and
/// 8192 bytes a Windows program once held.
const COUNTING: &str = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut count: usize = 0;
    mut bytes: usize = 0;
    mut last: &[u8] = \"\";
    for arg in sys.args() {
        count += 1;
        bytes += arg.len();
        last = arg;
    }
    if count == 602 and bytes > 10000 and last == \"last\" { .code(7) } else { .code(1) }
};
";

#[test]
fn a_windows_program_is_started_with_every_argument_its_command_line_holds() {
    for target in [targets::windows_x86_64::msvc, targets::windows_x86::msvc] {
        let fixture = Fixture::new(&format!("args-{}", target.symbol()));
        let (mut session, file) = backend::on(&fixture, "count.vun", COUNTING, target);
        if session.runner().is_empty() {
            continue;
        }
        let program = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
        let arguments = (0..600).map(|k| format!("argument-number-{k}")).chain(["last".to_owned()]);
        let status = session.command(&program).args(arguments).status().unwrap();
        assert_eq!(status.code(), Some(7), "{}", target.symbol());
    }
}
