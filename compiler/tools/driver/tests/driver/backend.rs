//! Code generation (spec 13, section 8): programs built through a backend, compiled and linked
//! by the target's tools, run.

use vun_harness::{ACCEPT, repo_root};

use vun_driver::compare::Ran;

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

/// The test program `name`, a file of `programs/` beside these tests, which
/// `vun debug compare` runs as the tests do.
fn program(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/driver/programs").join(name);
    std::fs::read_to_string(path).unwrap()
}

/// Builds `source` through each backend that generates code and runs it: its exit status, the
/// same through each (what `vun toolchain compare` shows).
fn status(name: &str, source: &str) -> Option<i32> {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let runs = session.run_each(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let statuses: Vec<(&str, Option<i32>)> =
        runs.iter().map(|r| (r.backend, r.output.status.code())).collect();
    assert!(Ran::agree(&runs), "the backends disagree: {statuses:?}");
    statuses[0].1
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
    assert_eq!(status("backend-status", &program("exit_status.vun")), Some(3));
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
    assert_eq!(status("backend-loops", &program("loops.vun")), Some((31 + 6 + 255) % 256));
}

#[test]
fn coroutines_generated_by_staged_loops_run_as_their_steps() {
    assert_eq!(status("backend-staged-frames", &program("staged_frames.vun")), Some(16));
}

#[test]
fn arithmetic_records_atomics_and_matches_run_as_they_mean() {
    // Division and remainder of each signedness, shifts, bit operations, signed comparisons,
    // nine parameters, records passed and returned in registers and in memory under the C
    // convention, atomics, a match on integers and a copy of 320 bytes: 24, as computed by hand.
    assert_eq!(status("backend-mix", &program("mix.vun")), Some(24));
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

#[test]
fn floats_compute_compare_and_pass_as_ieee_754_says() {
    // Each check that holds adds its bit: 255 when every one does.
    assert_eq!(status("backend-floats", &program("floats.vun")), Some(255));
}

#[test]
fn integers_wider_than_the_backend_run_as_pairs_of_halves() {
    assert_eq!(status("backend-wide", &program("wide.vun")), Some(255));
}

#[test]
fn closures_function_values_defers_and_primitives_lower_and_run() {
    assert_eq!(status("backend-lowered", &program("lowered.vun")), Some(255));
}
