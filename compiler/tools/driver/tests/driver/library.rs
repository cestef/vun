//! The toolchain's own sources, `lang` and `std`, check without a diagnostic: they obey every
//! rule the compiler enforces on user code.

use vun_driver::Shared;
use vun_fs::Tree;
use vun_harness::repo_root;
use vun_par::Parallel;
use vun_span::EXTENSION;

#[test]
fn library_sources_check_cleanly() {
    let paths = repo_root().join("lib").files_with(EXTENSION).unwrap();
    let shared = Shared::default();
    let reports = paths
        .par_map(|path| {
            let mut session = shared.session(path);
            let Some(file) = session.load(path) else { return Vec::new() };
            session.check(file);
            let sorted = session.diag.sorted(session.origins);
            sorted.into_iter().map(|d| format!("{}: {} {}", path.display(), d.id, d.message)).collect()
        })
        .concat();
    assert!(reports.is_empty(), "{reports:#?}");
}
