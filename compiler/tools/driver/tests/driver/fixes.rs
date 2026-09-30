//! `vun fix` over the rejected corpus (spec 10, section 7): the fixes of each file, applied to a
//! copy of it, never add errors, and applying fixes again changes nothing.

use vun_diag::Severity;
use vun_driver::Shared;
use vun_driver::fix::Applied;
use vun_fs::{Scratch, Tree};
use vun_harness::{REJECT, repo_root};
use vun_par::Parallel;
use vun_span::EXTENSION;

#[test]
fn fixing_the_rejected_corpus_adds_no_error_and_reaches_a_fixed_point() {
    let paths = repo_root().join(REJECT).files_with(EXTENSION).unwrap();
    let shared = Shared::default();
    let (failures, fixed): (Vec<Vec<String>>, Vec<usize>) = paths
        .par_map(|path| {
            let dir = Scratch::new("fix-corpus");
            let copy = dir.write(path.file_name().unwrap(), std::fs::read_to_string(path).unwrap());
            let mut session = shared.session(&copy);
            let Some(file) = session.load(&copy) else { return (Vec::new(), 0) };
            session.check(file);
            let before = std::mem::take(&mut session.diag).count(Severity::Error);
            let fixed = session.fix(file, Applied::Unsafe);
            let after = session.diag.count(Severity::Error);
            let mut failures = Vec::new();
            if after > before {
                failures.push(format!("{}: {before} errors became {after}", path.display()));
            }
            let count = fixed.iter().map(|f| f.fixes).sum();
            for f in &fixed {
                std::fs::write(&f.path, &f.text).unwrap();
            }
            let file = session.load(&copy).unwrap();
            if !session.fix(file, Applied::Unsafe).is_empty() {
                failures.push(format!("{}: fixing again changes it", path.display()));
            }
            (failures, count)
        })
        .into_iter()
        .unzip();
    let failures: Vec<_> = failures.concat();
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(fixed.iter().sum::<usize>() > 0, "no fix applies to the rejected corpus");
}
