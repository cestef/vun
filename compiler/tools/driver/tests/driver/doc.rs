//! API documentation through the driver: the namespaces of a package read as a documenter takes
//! them, with what only the compiler knows of their declarations.

use std::sync::Mutex;

use vun_component::Cause;
use vun_documenter::{Documenter, Site, Source, Types};

use vun_driver::{Components, Provider, Registry};

use crate::support::{Fixture, built_component};

/// Records whether the declarations it is told of are types, as the compiler says.
struct Probe(Mutex<Vec<(&'static str, bool)>>);

const DECLARED: [&str; 3] = ["Word", "count", "double"];

impl Documenter for Probe {
    fn name(&self) -> &'static str {
        "probe"
    }

    fn document(&self, sources: &[Source], types: &dyn Types) -> Result<Site, Cause> {
        let [source] = sources else { panic!("one namespace: {sources:?}") };
        let told = DECLARED.map(|name| (name, types.is_type(&source.path, name)));
        self.0.lock().unwrap().extend(told);
        Ok(Site { namespaces: sources.len(), files: Vec::new() })
    }
}

#[test]
fn a_declaration_whose_type_is_type_is_told_to_the_documenter_as_one() {
    let fixture = Fixture::new("doc-kinds");
    let manifest = Fixture::manifest("pkg", 1, "0.1.0");
    let dir = fixture.write(
        "pkg",
        &manifest,
        "\
/// A word.
Word := u32;
/// A count.
count := 3;
/// Doubles.
double := (x: u8) -> u16 { x * 2 };
",
    );
    let mut session = fixture.session();
    let probe = Probe(Mutex::default());
    let site = session.document(&probe, &[dir]).unwrap();
    assert_eq!(site.namespaces, 1);
    assert_eq!(*probe.0.lock().unwrap(), [("Word", true), ("count", false), ("double", false)]);
}

#[test]
fn a_documenter_loaded_from_a_library_documents_as_the_linked_one_does_asking_the_host() {
    let mut registry = Registry::default();
    registry.load(&Components::at(built_component("vun-component-documenter-html")));
    let loaded = registry.documenters.get("html").expect("the library serves a documenter");
    assert!(matches!(loaded.from, Provider::Loaded(_)));

    let fixture = Fixture::new("doc-loaded");
    let manifest = Fixture::manifest("pkg", 1, "0.1.0");
    let dir = fixture.write("pkg", &manifest, "/// A word.\nWord := u32;\n/// A count.\ncount := 3;\n");
    let mut session = fixture.session();
    let linked = session.document(&vun_doc::Html, std::slice::from_ref(&dir)).unwrap();
    let through = session.document(&*loaded.item, &[dir]).unwrap();
    assert_eq!(through, linked);
    assert!(through.files.iter().any(|w| w.contents.contains("Word")));
}
