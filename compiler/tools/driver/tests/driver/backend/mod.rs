//! Code generation (spec 13, section 8): programs built through a backend, compiled and linked
//! by the target's tools, run.

use vun_harness::{ACCEPT, repo_root};

use vun_driver::Codegen;
use vun_target::Name;

use crate::common::Fixture;

mod component;
mod run;

pub(crate) use run::{BACKENDS, build, on, run, sharing, status, status_across, status_on};

/// The test program `name`, a file of `programs/` beside these tests, which
/// `vun debug compare` runs as the tests do.
fn program(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/driver/programs").join(name);
    std::fs::read_to_string(path).unwrap()
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
    session.codegen = Codegen::through(vun_backend_c::INFO.id);
    let compiled = session.compile(file);
    let compiled = compiled.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert!(compiled.objects.len() > 2, "{:?}", compiled.objects);
    assert_eq!(compiled.frames.len(), compiled.coded.unit.functions.len());
    let program = session.build(file, None);
    let program = program.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let run = std::process::Command::new(program).output().unwrap();
    assert_eq!(run.stdout, b"hello\n");
}

#[test]
fn the_native_backend_reports_each_frame_it_lays_out() {
    let fixture = Fixture::new("backend-native-frames");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let path = fixture.root.write("hello.vun", text);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_native::INFO.id);
    let compiled = session.compile(file);
    let compiled = compiled.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(compiled.frames.len(), compiled.coded.unit.functions.len());
    // Every frame holds at least the return address and the saved frame pointer, but a naked
    // function's, which has none.
    let functions = &compiled.coded.unit.functions;
    let framed =
        compiled.frames.iter().filter(|&&(f, _)| functions[f].signature.conv != vun_lir::Conv::Naked);
    assert!(framed.clone().all(|&(_, bytes)| bytes >= 16), "{:?}", compiled.frames);
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
    std.os.Exit.of(total % 126)
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
fn compile_time_bindings_of_staged_loops_run_with_their_steps_types() {
    assert_eq!(status("backend-staged-bindings", &program("staged_bindings.vun")), Some(3 + 5));
}

#[test]
fn an_iterate_impl_leaves_the_result_of_a_coroutine_to_its_requirement() {
    assert_eq!(status("backend-iterated", &program("iterated.vun")), Some(10 + 1));
}

#[test]
fn code_with_no_value_at_compile_time_runs_where_it_is_spliced() {
    assert_eq!(status("backend-spliced-unrun", &program("spliced_unrun.vun")), Some(5));
}

#[test]
fn deferred_code_runs_at_run_time_though_it_has_a_value_at_compile_time() {
    assert_eq!(status("backend-spliced-deferred", &program("spliced_deferred.vun")), Some(TRAPPED));
}

#[test]
fn borrowed_constants_and_placed_fields_run_through_each_backend() {
    assert_eq!(status_across("backend-borrowed", &program("borrowed.vun")), Some(1 + 4 + 8 + 32));
}

#[test]
fn arithmetic_records_atomics_and_matches_run_as_they_mean() {
    // Division and remainder of each signedness, shifts, bit operations, signed comparisons,
    // nine parameters, records passed and returned in registers and in memory under the C
    // convention, atomics, a match on integers and a copy of 320 bytes: 24, as computed by hand.
    assert_eq!(status_across("backend-mix", &program("mix.vun")), Some(24));
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
    assert_eq!(status_across("backend-assert-holds", &checking("assert(n == 1);")), Some(7));
    assert_eq!(status_across("backend-assert-fails", &checking("assert(n > 1);")), Some(TRAPPED));
    let expect = "o: ?usize = if n > 1 { .some(n) } else { .none }; _ = o.expect(\"one argument\");";
    assert_eq!(status_across("backend-expect", &checking(expect)), Some(TRAPPED));
}

/// The ids of what building `source` through `backend` reports.
fn reported(name: &str, source: &str, backend: Name) -> Vec<vun_diag::DiagId> {
    let fixture = Fixture::new(&format!("{name}-{backend}"));
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(backend);
    let _ = session.compile(file);
    session.diag.sorted(session.origins).into_iter().map(|d| d.id.clone()).collect()
}

#[test]
fn stack_bounds_hold_over_the_frames_each_backend_reports() {
    use vun_diag::errors;
    let source = "\
#[link.export(\"fill\"), abi.c, stack.max(64)]
fill := (seed: u8) -> u8 { buf: [256]u8 = [seed; 256]; copy := buf; copy[0] };
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
    assert_eq!(status_across("backend-floats", &program("floats.vun")), Some(0));
}

#[test]
fn integers_wider_than_the_backend_run_as_pairs_of_halves() {
    assert_eq!(status_across("backend-wide", &program("wide.vun")), Some(0));
}

#[test]
fn integers_wrap_into_ranges_of_any_size() {
    assert_eq!(status_across("backend-wrapped", &program("wrapped.vun")), Some(0));
}

#[test]
fn vectors_compute_lane_by_lane_and_reduce_in_the_order_stated() {
    assert_eq!(status_across("backend-vectors", &program("vectors.vun")), Some(0));
}

#[test]
fn scalable_vectors_cover_a_length_that_is_no_multiple_of_their_width() {
    assert_eq!(status_across("backend-scalable", &program("scalable.vun")), Some(0));
}

#[test]
fn closures_function_values_defers_and_primitives_lower_and_run() {
    assert_eq!(status_across("backend-lowered", &program("lowered.vun")), Some(0));
}

#[test]
fn memory_operations_run_whether_their_value_is_kept_or_not() {
    assert_eq!(status_across("backend-memory", &program("memory.vun")), Some(0));
}

#[test]
fn a_call_of_a_function_of_dollar_parameters_alone_gives_its_integer_at_compile_time() {
    // The number of checks that fail: 0 when all hold.
    assert_eq!(status("backend-compile-time-calls", &program("compile_time_calls.vun")), Some(0));
}

#[test]
fn a_method_taking_its_receiver_by_value_is_called_through_a_borrow() {
    // The number of checks that fail: 0 when all hold.
    assert_eq!(status("backend-borrowed-receiver", &program("borrowed_receiver.vun")), Some(0));
}

#[test]
fn data_aligned_past_every_c_type_is_aligned() {
    assert_eq!(status_across("backend-aligned", &program("aligned.vun")), Some(7));
}

#[test]
fn float_rem_and_mul_add_run_as_vun_and_agree_with_compile_time_bit_for_bit() {
    // The number of cases whose run-time result differs from the compile-time one.
    assert_eq!(status_across("backend-soft-float", &program("soft_float.vun")), Some(0));
}

#[test]
fn f16_runs_as_bits_computed_in_f32_and_agrees_with_compile_time_bit_for_bit() {
    // The number of checks whose run-time result differs from the compile-time one.
    assert_eq!(status_across("backend-half", &program("half.vun")), Some(0));
}

#[test]
fn the_native_backend_lists_its_code_under_the_source_lines() {
    let fixture = Fixture::new("backend-native-listing");
    let path = fixture.root.write("main.vun", program("loops.vun"));
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_native::INFO.id);
    let mut out = Vec::new();
    session.dump_code(file, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    // A frame's prologue, as the host's instruction set writes it.
    let prologue = if cfg!(target_arch = "aarch64") { "stp x29, x30" } else { "push rbp" };
    assert!(text.lines().any(|l| l.contains(prologue)), "{text}");
    assert!(text.lines().any(|l| l.starts_with("  ; ") && l.contains("main.vun:")), "{text}");
}
