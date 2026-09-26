//! Every function of the repository that MIR lowers verifies (spec 13, section 4).

use vun_driver::Session;
use vun_fs::Tree;
use vun_harness::{ACCEPT, SOURCES, repo_root};
use vun_par::Parallel;
use vun_span::EXTENSION;

#[test]
fn repository_bodies_lower_to_mir_that_verifies() {
    let root = repo_root();
    let paths: Vec<_> =
        SOURCES.iter().flat_map(|dir| root.join(dir).files_with(EXTENSION).unwrap()).collect();
    let checked = paths.par_map(|path| {
        let (mut failures, mut lowered) = (Vec::new(), 0);
        let mut session = Session::new();
        let Some(file) = session.load(path) else { return (failures, lowered) };
        for (_, body) in session.bodies(file) {
            let Ok(body) = body else { continue };
            lowered += 1;
            if let Err(malformed) = body.verify(&session.origins) {
                failures.push(format!("{}: {malformed}\n{body}", path.display()));
            }
        }
        (failures, lowered)
    });
    let lowered: usize = checked.iter().map(|(_, n)| n).sum();
    let failures: Vec<_> = checked.into_iter().flat_map(|(f, _)| f).collect();
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(lowered > 0, "no body lowered");
}

#[test]
fn every_function_a_corpus_program_reaches_is_concrete() {
    let paths = repo_root().join(ACCEPT).files_with(EXTENSION).unwrap();
    let programs = paths.par_map(|path| {
        let mut session = Session::new();
        let Some(file) = session.load(path) else { return (Vec::new(), 0) };
        let program = session.program(file);
        let unready = program.unready.iter().map(|(_, why)| format!("{}: {why}", path.display()));
        (unready.collect::<Vec<_>>(), program.functions.len())
    });
    let reached: usize = programs.iter().map(|(_, n)| n).sum();
    let unready: Vec<_> = programs.into_iter().flat_map(|(u, _)| u).collect();
    assert!(unready.is_empty(), "{unready:#?}");
    assert!(reached > 0, "no corpus program exports anything");
}
