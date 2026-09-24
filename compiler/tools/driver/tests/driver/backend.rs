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
