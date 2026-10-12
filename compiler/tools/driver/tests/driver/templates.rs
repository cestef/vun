//! Starting a project from a template of the library (`vun new`).

use std::path::Path;

use vun_build::layout::MANIFEST;
use vun_diag::errors;
use vun_driver::packages::Library;
use vun_fs::Tree;

use crate::common::Fixture;

#[test]
fn a_template_is_copied_with_the_name_of_its_directory_and_an_id_of_its_own() {
    let fixture = Fixture::new("new-copy");
    let dir = fixture.root.join("mine");
    let mut session = fixture.session();
    let Ok(made) = session.new_project(&dir, Library::DEFAULT_TEMPLATE) else {
        panic!("the template is there")
    };
    assert!(made.iter().any(|file| file.as_os_str() == MANIFEST));
    let manifest = session.manifest_at(&dir.join(MANIFEST)).expect("the copy declares a package");
    assert_eq!(manifest.name, "mine");
    assert_ne!(manifest.id, 0, "the template's placeholder is replaced");
}

#[test]
fn a_template_names_its_files_after_the_package_and_has_a_namespace_to_test() {
    let fixture = Fixture::new("new-renamed");
    let dir = fixture.root.join("mine");
    let mut session = fixture.session();
    let made = session.new_project(&dir, Library::DEFAULT_TEMPLATE).expect("the template is there");
    let root = Path::new("mine.vun");
    assert!(
        made.iter().any(|file| file == root) && made.iter().any(|file| file == Path::new("mine/tests.vun"))
    );
    let manifest = session.manifest_at(&dir.join(MANIFEST)).expect("the copy declares a package");
    assert_eq!(manifest.include, ["build.vun", "main.vun", "mine.vun", "mine"]);
    let file = session.load(&dir.join(root)).unwrap();
    assert!(session.tested(file, Some(&dir.join(root))).is_some(), "the package has a namespace to test");
}

#[test]
fn a_project_is_never_written_over() {
    let fixture = Fixture::new("new-exists");
    let dir = fixture.root.join("mine");
    let mut session = fixture.session();
    assert!(session.new_project(&dir, Library::DEFAULT_TEMPLATE).is_ok());
    assert!(session.new_project(&dir, Library::DEFAULT_TEMPLATE).is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::NEW_EXISTS]);
}

#[test]
fn a_template_the_library_lacks_is_refused() {
    let fixture = Fixture::new("new-missing");
    let mut session = fixture.session();
    assert!(session.new_project(&fixture.root.join("mine"), "no-such-template").is_err());
    let ids: Vec<_> = session.diag.sorted(session.origins).iter().map(|d| d.id).collect();
    assert_eq!(ids, [errors::NEW_NO_TEMPLATE]);
}

#[test]
fn every_template_is_a_project_that_checks() {
    let fixture = Fixture::new("new-checks");
    let templates = Library::templates(&Library::find().dir);
    assert!(!templates.is_empty());
    for template in templates {
        // Named as the template, a project would hide the package its template depends on.
        let dir = fixture.root.join(format!("project_{template}"));
        let mut session = fixture.session();
        assert!(session.new_project(&dir, &template).is_ok(), "{template} is copied");
        for file in dir.files_with("vun").unwrap() {
            let mut session = fixture.session();
            let loaded = session.load(&file).unwrap();
            session.check_plan(loaded);
            session.check(loaded);
            let reported: Vec<_> =
                session.diag.sorted(session.origins).iter().map(|d| (d.id, d.message.to_string())).collect();
            assert_eq!(reported, [], "{template}: {}", file.display());
        }
    }
}

#[test]
fn a_directory_no_code_can_name_is_refused_for_a_package() {
    let fixture = Fixture::new("new-bad-name");
    let mut session = fixture.session();
    assert!(session.new_project(&fixture.root.join("gtk-todo"), Library::DEFAULT_TEMPLATE).is_err());
    let found = session.diag.sorted(session.origins);
    assert_eq!(found.iter().map(|d| d.id).collect::<Vec<_>>(), [errors::NEW_BAD_NAME]);
    assert!(!fixture.root.join("gtk-todo").exists(), "nothing is written for a name refused");
    let mut again = fixture.session();
    assert!(again.new_project(&fixture.root.join("gtk_todo"), Library::DEFAULT_TEMPLATE).is_ok());
}
