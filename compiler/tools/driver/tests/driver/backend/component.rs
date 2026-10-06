//! Backends as components (decision 498): the Cranelift backend loaded from its library builds
//! the programs the linked one does, through the boundary, and what it cannot express (the
//! system calls of a hosted program among it) is compiled by the native backend, whichever way
//! that is provided.

use std::process::Command;

use vun_driver::{Codegen, Components, Registry, Session};
use vun_harness::{ACCEPT, repo_root};

use crate::common::{Fixture, components_of};

/// The crates of the components the tests load.
const CRANELIFT: &str = "vun-component-backend-cranelift";
const NATIVE: &str = "vun-component-backend-native";

/// A session whose backends are the libraries of `packages` alone, building `source`.
fn through(fixture: &Fixture, source: &str, packages: &[&str]) -> (Session, vun_span::FileId) {
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.registry = Registry::default();
    session.registry.load(&Components::at(components_of(fixture, packages)));
    session.registry.add_linker(Box::new(vun_link_gnu::GnuLinker));
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(vun_backend_cranelift::INFO.id);
    (session, file)
}

#[test]
fn a_backend_loaded_from_a_library_builds_a_program_that_runs() {
    let fixture = Fixture::new("component-hello");
    let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs/hello.vun")).unwrap();
    let (mut session, file) = through(&fixture, &text, &[CRANELIFT, NATIVE]);
    let loaded = session.registry.backends().map(|b| b.info().id).collect::<Vec<_>>();
    let libraries = [vun_backend_cranelift::INFO.id, vun_backend_native::INFO.id];
    assert_eq!(loaded, libraries, "the libraries are the only backends");
    let program = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let output = Command::new(program).output().unwrap();
    assert_eq!(output.stdout, b"hello\n");
}

#[test]
fn what_a_backend_cannot_express_is_compiled_by_the_assisting_backend_the_host_has() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    let fixture = Fixture::new("component-assist");
    let (mut session, file) = through(&fixture, crate::asm::NAKED, &[CRANELIFT, NATIVE]);
    let program = session.build(file, None).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    assert_eq!(Command::new(program).status().unwrap().code(), Some(7));

    let alone = Fixture::new("component-unassisted");
    let (mut session, file) = through(&alone, crate::asm::NAKED, &[CRANELIFT]);
    assert!(session.build(file, None).is_err(), "nothing assists it");
}
