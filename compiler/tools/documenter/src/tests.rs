use vun_component::{Cause, Component};

use crate::{Documenter, Documenters, Remote, Site, Source, Types, Written};

/// Writes a file per namespace, telling what the host says of its `Word` declaration.
struct Stub;

impl Documenter for Stub {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn document(&self, sources: &[Source], types: &dyn Types) -> Result<Site, Cause> {
        assert!(!sources.is_empty(), "told to");
        let files = sources.iter().map(|source| Written {
            path: source.file.clone(),
            contents: format!("{}:{}", source.text, types.is_type(&source.path, "Word")),
        });
        Ok(Site { namespaces: sources.len(), files: files.collect() })
    }
}

/// Knows that `Word` is a type in `a` alone.
struct Known;

impl Types for Known {
    fn is_type(&self, path: &[String], name: &str) -> bool {
        path == ["a"] && name == "Word"
    }
}

fn remote() -> Remote {
    Remote::of(Component::within(vun_component::descriptor!(Documenters, Stub)).unwrap()).unwrap()
}

fn source(path: &str, text: &str) -> Source {
    Source { path: vec![path.to_owned()], file: format!("{path}.vun"), text: text.to_owned() }
}

#[test]
fn the_documentation_arrives_with_what_the_documenter_asked_the_host() {
    let remote = remote();
    assert_eq!(remote.name(), "stub");
    let site = remote.document(&[source("a", "x"), source("b", "y")], &Known).unwrap();
    assert_eq!(site.namespaces, 2);
    let contents: Vec<_> = site.files.iter().map(|w| (w.path.as_str(), w.contents.as_str())).collect();
    assert_eq!(contents, [("a.vun", "x:true"), ("b.vun", "y:false")]);
}

#[test]
fn a_documenter_that_panics_is_a_failure_with_its_cause_and_is_called_again() {
    let remote = remote();
    let failure = remote.document(&[], &Known).unwrap_err();
    assert!(failure.to_string().contains("told to"), "{failure}");
    assert!(remote.document(&[source("a", "x")], &Known).is_ok());
}
