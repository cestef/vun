//! Backends as components (decision 498): the Cranelift backend loaded from its library builds
//! the programs the linked one does, through the boundary.

use std::process::Command;

use vun_driver::{Codegen, Components, Registry};
use vun_harness::{ACCEPT, repo_root};

use crate::common::{Fixture, built_component};

/// The crate of the component the tests load.
const CRATE: &str = "vun-component-backend-cranelift";

#[test]
fn a_backend_loaded_from_a_library_builds_a_program_that_runs() {
    let fixture = Fixture::new("component-hello");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let path = fixture.root.write("hello.vun", text);
    let mut session = fixture.session();
    session.registry = Registry::default();
    session.registry.load(&Components::at(built_component(CRATE)));
    session.registry.add_linker(Box::new(vun_link_gnu::GnuLinker));
    let loaded = session.registry.backends().map(|b| b.info().id).collect::<Vec<_>>();
    assert_eq!(loaded, [vun_backend_cranelift::INFO.id], "the library is the only backend");
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_cranelift::INFO.id);
    let program = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let output = Command::new(program).output().unwrap();
    assert_eq!(output.stdout, b"hello\n");
}
