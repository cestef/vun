//! Every function of the repository that MIR lowers verifies (spec 13, section 4).

use vun_driver::Session;
use vun_fs::Tree;
use vun_harness::{ACCEPT, SOURCES, repo_root};
use vun_span::EXTENSION;

#[test]
fn repository_bodies_lower_to_mir_that_verifies() {
    let root = repo_root();
    let (mut failures, mut lowered) = (Vec::new(), 0);
    for dir in SOURCES {
        for path in root.join(dir).files_with(EXTENSION).unwrap() {
            let mut session = Session::new();
            let Some(file) = session.load(&path) else { continue };
            for (_, body) in session.bodies(file) {
                let Ok(body) = body else { continue };
                lowered += 1;
                if let Err(malformed) = body.verify(&session.origins) {
                    failures.push(format!("{}: {malformed}\n{body}", path.display()));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(lowered > 0, "no body lowered");
}

#[test]
fn every_function_a_corpus_program_reaches_is_concrete() {
    let (mut unready, mut reached) = (Vec::new(), 0);
    for path in repo_root().join(ACCEPT).files_with(EXTENSION).unwrap() {
        let mut session = Session::new();
        let Some(file) = session.load(&path) else { continue };
        let program = session.program(file);
        reached += program.functions.len();
        unready.extend(program.unready.iter().map(|(_, why)| format!("{}: {why}", path.display())));
    }
    assert!(unready.is_empty(), "{unready:#?}");
    assert!(reached > 0, "no corpus program exports anything");
}
