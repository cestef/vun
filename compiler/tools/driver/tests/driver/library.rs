//! The toolchain's own sources, `lang` and `std`, check without a diagnostic: they obey every
//! rule the compiler enforces on user code.

use vun_driver::Session;
use vun_fs::Tree;
use vun_harness::repo_root;
use vun_par::Parallel;
use vun_span::EXTENSION;

#[test]
fn library_sources_check_cleanly() {
    let root = repo_root();
    let paths = root.join("lib").files_with(EXTENSION).unwrap();
    let reports: Vec<String> = paths
        .par_map(|path| {
            let mut session = Session::new();
            let Some(file) = session.load(path) else { return Vec::new() };
            session.check(file);
            let reports = session
                .diag
                .sorted()
                .into_iter()
                .map(|d| format!("{}: {} {}", path.display(), d.id, d.message));
            reports.collect::<Vec<_>>()
        })
        .into_iter()
        .flatten()
        .collect();
    assert!(reports.is_empty(), "{reports:#?}");
}
