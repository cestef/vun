//! The toolchain's own sources, `lang` and `std`, check without a diagnostic: they obey every
//! rule the compiler enforces on user code.

use vun_driver::Session;
use vun_fs::Tree;
use vun_harness::repo_root;
use vun_span::EXTENSION;

#[test]
fn library_sources_check_cleanly() {
    let root = repo_root();
    let mut reports = Vec::new();
    for path in root.join("lib").files_with(EXTENSION).unwrap() {
        let mut session = Session::new();
        let Some(file) = session.load(&path) else { continue };
        session.check(file);
        for d in session.diag.sorted() {
            reports.push(format!("{}: {} {}", path.display(), d.id, d.message));
        }
    }
    assert!(reports.is_empty(), "{reports:#?}");
}
