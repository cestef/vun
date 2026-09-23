use std::path::Path;

use super::Tree;

#[test]
fn walks_are_sorted_and_a_file_is_its_own_walk() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let names = |paths: Vec<std::path::PathBuf>| -> Vec<String> {
        paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect()
    };
    assert_eq!(names(src.files().unwrap()), ["lib.rs", "tests.rs"]);
    assert_eq!(names(src.join("lib.rs").files().unwrap()), ["lib.rs"]);
    assert!(src.join("absent").files().is_err());
}
