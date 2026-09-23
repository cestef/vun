//! Namespaces on disk: a file and the directory of the same name are one namespace.

mod common;

use common::Fixture;
use overt_diag::errors;

#[test]
fn names_in_both_a_file_and_its_directory_are_duplicates() {
    let fx = Fixture::new("members");
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.include.push(app.name.clone());
    let dir = fx.write("", &app, "uart := 1;\ngpio := 2;\n");
    fx.root.write(dir.join(&app.name).join("uart.ov"), "");
    assert_eq!(Fixture::check(&Fixture::file(&dir, &app)), [errors::NAMES_DUPLICATE]);
}
