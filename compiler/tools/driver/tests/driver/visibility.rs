//! Restricted names and the scopes `#[visible(...)]` widens them to (spec 01, section 7).

use std::path::{Path, PathBuf};

use crate::common::Fixture;
use vun_diag::{DiagId, errors};
use vun_span::EXTENSION;

/// A package `app` holding `app.net.wire`, which declares a name per scope, and `app.util`.
struct Scopes {
    fx: Fixture,
    dir: PathBuf,
}

impl Scopes {
    fn new() -> Self {
        let fx = Fixture::new("visibility");
        let mut app = Fixture::manifest("app", 1, "0.1.0");
        app.include.push(app.name.clone());
        let dir = fx.write("", &app, "outside := app.net.wire._net;\npackage := app.net.wire._package;\n");
        let scopes = Self { fx, dir };
        scopes.put(&["net"], "inside := app.net.wire._net;\nhidden := app.net.wire._wire;\n");
        scopes.put(&["util"], "");
        scopes.put(
            &["net", "wire"],
            "\
_wire := 1;
#[visible(.within(app.net))]
_net := 2;
#[visible(.package)]
_package := 3;
#[visible(.within(app.util))]
_elsewhere := 4;
#[visible(.package)]
open := 5;
",
        );
        scopes
    }

    /// The file of the namespace `app.<path>`.
    fn file(&self, path: &[&str]) -> PathBuf {
        let mut file = self.dir.join("app");
        file.extend(path);
        file.with_extension(EXTENSION)
    }

    fn put(&self, path: &[&str], source: &str) {
        self.fx.root.write(self.file(path), source);
    }

    fn check(&self, file: &Path) -> Vec<DiagId> {
        self.fx.check(file)
    }
}

#[test]
fn a_restricted_name_is_visible_inside_its_scope_only() {
    let scopes = Scopes::new();
    assert_eq!(scopes.check(&scopes.dir.join("app").with_extension(EXTENSION)), [errors::NAMES_PRIVATE]);
    assert_eq!(scopes.check(&scopes.file(&["net"])), [errors::NAMES_PRIVATE]);
}

#[test]
fn a_scope_widens_only_restricted_names_and_encloses_them() {
    let scopes = Scopes::new();
    assert_eq!(
        scopes.check(&scopes.file(&["net", "wire"])),
        [errors::NAMES_VISIBILITY, errors::NAMES_VISIBILITY]
    );
}
