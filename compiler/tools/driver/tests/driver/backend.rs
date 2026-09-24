//! Code generation (spec 13, section 8): programs built through a backend, compiled and linked
//! by the target's tools, run.

use vun_harness::{ACCEPT, repo_root};

use crate::common::Fixture;

#[test]
fn a_hosted_program_built_through_c_runs() {
    let fixture = Fixture::new("backend-c-hello");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let path = fixture.root.write("hello.vun", text);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.build(file, Some(vun_backend_c::INFO.id.0), None).unwrap();
    let run = std::process::Command::new(program).output().unwrap();
    assert!(run.status.success());
    assert_eq!(run.stdout, b"hello\n");
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
fn a_program_exits_with_the_status_it_returns() {
    let fixture = Fixture::new("backend-c-status");
    let path = fixture.root.write(
        "status.vun",
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit { .code(3) };
",
    );
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.build(file, Some(vun_backend_c::INFO.id.0), None).unwrap();
    let run = std::process::Command::new(program).status().unwrap();
    assert_eq!(run.code(), Some(3));
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

/// Builds `source` through the C backend and runs it, giving its exit status.
fn status(name: &str, source: &str) -> Option<i32> {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    let program = session.build(file, Some(vun_backend_c::INFO.id.0), None);
    let program = program.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    std::process::Command::new(program).status().unwrap().code()
}

#[test]
fn a_loop_resumes_a_coroutine_to_its_end() {
    assert_eq!(status("backend-c-frames", &driving("total ^= x;")), Some(0x03 ^ 0x04 ^ 0x05 ^ 0x1e));
}

#[test]
fn breaking_out_of_a_loop_ends_the_suspended_frame() {
    assert_eq!(status("backend-c-break", &driving("if x == 5 { break; } total ^= x;")), Some(0x03 ^ 0x04));
}
