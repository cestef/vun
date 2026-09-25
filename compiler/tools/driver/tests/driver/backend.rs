//! Code generation (spec 13, section 8): programs built through a backend, compiled and linked
//! by the target's tools, run.

use vun_harness::{ACCEPT, repo_root};

use crate::common::Fixture;

/// The backends every program here is built through, which must agree.
const BACKENDS: [&str; 2] = [vun_backend_c::INFO.id.0, vun_backend_baseline::INFO.id.0];

/// Builds `source` through `backend` and runs it.
fn run(name: &str, source: &str, backend: &str) -> std::process::Output {
    let fixture = Fixture::new(&format!("{name}-{backend}"));
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.build(file, Some(backend), None);
    let program = program.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    std::process::Command::new(program).output().unwrap()
}

/// Builds `source` through each backend and runs it: its exit status, the same through each.
fn status(name: &str, source: &str) -> Option<i32> {
    let statuses: Vec<Option<i32>> = BACKENDS.iter().map(|b| run(name, source, b).status.code()).collect();
    assert!(statuses.windows(2).all(|w| w[0] == w[1]), "{BACKENDS:?} disagree: {statuses:?}");
    statuses[0]
}

#[test]
fn a_hosted_program_runs_through_each_backend() {
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    for backend in BACKENDS {
        let run = run("backend-hello", &text, backend);
        assert!(run.status.success());
        assert_eq!(run.stdout, b"hello\n");
    }
}

#[test]
fn a_program_split_into_shards_runs_and_reports_its_frames() {
    let fixture = Fixture::new("backend-c-shards");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let path = fixture.root.write("hello.vun", text);
    let mut session = fixture.session();
    session.registry = vun_driver::Registry::default();
    session.registry.add_backend(Box::new(vun_backend_c::CBackend { shard: 4 }));
    session.registry.add_linker(Box::new(vun_link_gnu::GnuLinker));
    let file = session.load(&path).unwrap();
    let compiled = session.compile(file, Some(vun_backend_c::INFO.id.0));
    let compiled = compiled.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert!(compiled.objects.len() > 2, "{:?}", compiled.objects);
    assert_eq!(compiled.frames.len(), compiled.coded.unit().functions.len());
    let program = session.build(file, Some(vun_backend_c::INFO.id.0), None);
    let program = program.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let run = std::process::Command::new(program).output().unwrap();
    assert_eq!(run.stdout, b"hello\n");
}

#[test]
fn the_baseline_backend_reports_each_frame_it_lays_out() {
    let fixture = Fixture::new("backend-baseline-frames");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let path = fixture.root.write("hello.vun", text);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let compiled = session.compile(file, Some(vun_backend_baseline::INFO.id.0));
    let compiled = compiled.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(compiled.frames.len(), compiled.coded.unit().functions.len());
    // Every frame holds at least the return address and the saved frame pointer.
    assert!(compiled.frames.iter().all(|&(_, bytes)| bytes >= 16), "{:?}", compiled.frames);
}

#[test]
fn a_program_exits_with_the_status_it_returns() {
    let source = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .code(3) };
";
    assert_eq!(status("backend-status", source), Some(3));
}

/// A program driving a coroutine of its own, `values`, and exiting with what `body` computes
/// into `total` from each value `x`.
fn driving(body: &str) -> String {
    format!(
        "\
start := std.os.start(main);
values := () -> std.iter.Iter(u8) {{ yield 3; yield 4; yield 5; yield 30; }};
main := (sys: &mut std.os.System) -> std.os.Exit {{
    mut total: u8 = 0;
    for x in values() {{ {body} }}
    .code(total)
}};
"
    )
}

#[test]
fn a_loop_resumes_a_coroutine_to_its_end() {
    assert_eq!(status("backend-frames", &driving("total ^= x;")), Some(0x03 ^ 0x04 ^ 0x05 ^ 0x1e));
}

#[test]
fn breaking_out_of_a_loop_ends_the_suspended_frame() {
    assert_eq!(status("backend-break", &driving("if x == 5 { break; } total ^= x;")), Some(0x03 ^ 0x04));
}

#[test]
fn loops_over_slices_arrays_and_bounds_count_within_their_items() {
    let source = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    xs: [5]u8 = [1, 2, 4, 8, 16];
    mut total: u8 = 0;
    for x in &xs { total = (total + x.*).wrap(u8); }
    for x in &xs[1..=2] { total = (total + x.*).wrap(u8); }
    n: u8 = sys.args().len().sat(u8);
    last: u8 = (n + 254).sat(u8);
    mut count: u8 = 0;
    for _ in 0..=last { count = (count + 1).sat(u8); }
    for _ in n..n { count = 0; }
    .code((total + count).wrap(u8))
};
";
    assert_eq!(status("backend-loops", source), Some((31 + 6 + 255) % 256));
}

#[test]
fn coroutines_generated_by_staged_loops_run_as_their_steps() {
    let source = "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut total: u8 = 0;
    $for k in [3, 5] {
        gen := () -> std.iter.Iter(u8) { yield k; yield k; };
        for x in gen() { total = (total + x).wrap(u8); }
    }
    .code(total)
};
";
    assert_eq!(status("backend-staged-frames", source), Some(16));
}

#[test]
fn arithmetic_records_atomics_and_matches_run_as_they_mean() {
    // Division and remainder of each signedness, shifts, bit operations, signed comparisons,
    // nine parameters, records passed and returned in registers and in memory under the C
    // convention, atomics, a match on integers and a copy of 320 bytes: 24, as computed by hand.
    let source = "\
start := std.os.start(main);

mix := (a: u8, b: u16, c: u32, d: u64, e: i8, f: i16, g: i32, h: u8, k: Int(1..=255)) -> u64 {
    q := d / 7;
    r := d % 10;
    s := g / 3;
    t := g % 5;
    u := (c >> 3) ^ (c << 2);
    v := (a & 0x0f) | (b.wrap(u8) ^ h);
    w := d / k;
    neg: u8 = if e < 0 { 1 } else { 2 };
    sign: u8 = if f <= -2 { 4 } else { 8 };
    base := q.wrap(u8) + r.wrap(u8) + u.wrap(u8) + v + w.wrap(u8) + neg + sign;
    base + (s + t + 1000).sat(u16)
};

#[layout.c]
Pair := struct { x: u32, y: u64 };
#[layout.c]
Big := struct { a: u64, b: u64, c: u64 };

#[abi.c]
swap := (p: Pair) -> Pair { Pair{ .x = p.y.wrap(u32), .y = p.x } };
#[abi.c]
spin := (b: Big) -> Big { Big{ .a = b.c, .b = b.a, .c = b.b } };

pick := (x: u8) -> u8 {
    match x {
        0 => 10,
        1 => 20,
        7 => 30,
        _ => 40,
    }
};

main := (sys: &mut std.os.System) -> std.os.Exit {
    n := sys.args().len();
    n8: u8 = n.sat(u8);
    m := mix(n8, 700, 123456, 99999999999, -3, -2, -77, 5, 3);
    p := swap(Pair{ .x = 1, .y = 2 });
    big := spin(Big{ .a = 1, .b = 2, .c = 3 });
    counter := std.sync.Atomic(u32).init(5);
    old := counter.add(3, .seq_cst);
    was := counter.swap(10, .seq_cst);
    cas := counter.compare_swap(10, 11, .seq_cst);
    now := counter.load(.acquire);
    arr: [40]u64 = [7; 40];
    copy := arr;
    x := (m.wrap(u8) + p.y.wrap(u8) + p.x.wrap(u8) + (big.a.wrap(u8) * 3 + big.b.wrap(u8) * 5 + big.c.wrap(u8) * 7).wrap(u8)
        + pick(n8) + pick((n8 + 6).wrap(u8)) + old.wrap(u8) + was.wrap(u8) + cas.wrap(u8) + now.wrap(u8) + copy[39].wrap(u8)).wrap(u8);
    .code(x)
};
";
    assert_eq!(status("backend-mix", source), Some(24));
}

/// The status the C library runtime exits with when a program traps (`std.os.libc.trapped`).
const TRAPPED: i32 = 101;

/// A program checking the number of its arguments, one here, with `check`.
fn checking(check: &str) -> String {
    format!(
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    n := sys.args().len();
    {check}
    .code(7)
}};
"
    )
}

#[test]
fn assertions_and_failures_trap_at_run_time() {
    assert_eq!(status("backend-assert-holds", &checking("assert(n == 1);")), Some(7));
    assert_eq!(status("backend-assert-fails", &checking("assert(n > 1);")), Some(TRAPPED));
    let expect = "o: ?usize = if n > 1 { .some(n) } else { .none }; _ = o.expect(\"one argument\");";
    assert_eq!(status("backend-expect", &checking(expect)), Some(TRAPPED));
}

/// The ids of what building `source` through `backend` reports.
fn reported(name: &str, source: &str, backend: &str) -> Vec<vun_diag::DiagId> {
    let fixture = Fixture::new(&format!("{name}-{backend}"));
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let _ = session.compile(file, Some(backend));
    session.diag.sorted().into_iter().map(|d| d.id.clone()).collect()
}

#[test]
fn stack_bounds_hold_over_the_frames_each_backend_reports() {
    use vun_diag::errors;
    let source = "\
#[link.export(\"fill\"), abi.c, stack.max(64)]
fill := (seed: u8) -> u8 { buf: [256]u8 = [seed; 256]; copy := buf; seed };
#[link.export(\"small\"), abi.c, stack.max(512)]
small := (x: u8) -> u8 { x };
count := (n: u32) -> u32 { if n == 0 { 0 } else { (1 + count(n - 1)).sat(u32) } };
#[link.export(\"depth\"), abi.c, stack.max(512)]
depth := (n: u32) -> u32 { count(n) };
#[link.symbol(\"tick\"), abi.c, stack.max(32)]
tick: (x: u8) -> u8 = _;
#[link.symbol(\"tock\"), abi.c]
tock: (x: u8) -> u8 = _;
#[link.export(\"declared\"), abi.c, stack.max(4096)]
declared := (x: u8) -> u8 { tick(x) };
#[link.export(\"undeclared\"), abi.c, stack.max(4096)]
undeclared := (x: u8) -> u8 { tock(x) };
#[link.symbol(lang.panic.symbols.trap), stack.max(128)]
trap: (t: &lang.panic.Trap) -> lang.never = _;
#[link.export(\"trapping\"), abi.c, stack.max(4096)]
trapping := (n: u32) -> u32 { mut total: u32 = 0; for _ in 0..n { total += 1; } total };
";
    for backend in BACKENDS {
        let ids = reported("backend-stack", source, backend);
        let expected = [errors::STACK_EXCEEDED, errors::STACK_UNBOUNDED, errors::STACK_UNBOUNDED];
        assert_eq!(ids, expected, "{backend}");
    }
}
