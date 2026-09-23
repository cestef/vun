//! Namespaces on disk: a file and the directory of the same name are one namespace.

use overt_diag::errors;
use overt_driver::Session;
use overt_fs::Scratch;

#[test]
fn names_in_both_a_file_and_its_directory_are_duplicates() {
    let root = Scratch::new("members");
    root.write(
        "build.ov",
        "package := .{ .name = \"app\", .id = 1, .version = \"0.1.0\", .edition = 2027,\n\
         .include = .{ \"build.ov\", \"app.ov\", \"app\" } };\n",
    );
    root.write("app.ov", "uart := 1;\ngpio := 2;\n");
    root.write("app/uart.ov", "");
    let mut session = Session::new();
    let file = session.load(&root.join("app.ov")).unwrap();
    session.check(file);
    let found: Vec<_> =
        session.diag.sorted().into_iter().map(|d| (d.id.clone(), d.message.to_string())).collect();
    assert!(
        matches!(found.as_slice(), [(id, msg)] if *id == errors::NAMES_DUPLICATE && msg.contains("uart")),
        "{found:?}"
    );
}
