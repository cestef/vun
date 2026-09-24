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
