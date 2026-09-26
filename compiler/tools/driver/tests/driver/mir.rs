//! Every function of the repository that MIR lowers verifies (spec 13, section 4).

use vun_driver::Shared;
use vun_fs::Tree;
use vun_harness::{ACCEPT, SOURCES, repo_root};
use vun_par::Parallel;
use vun_span::EXTENSION;

#[test]
fn repository_bodies_lower_to_mir_that_verifies() {
    let root = repo_root();
    let paths: Vec<_> =
        SOURCES.iter().flat_map(|dir| root.join(dir).files_with(EXTENSION).unwrap()).collect();
    let shared = Shared::default();
    let (failures, lowered): (Vec<Vec<String>>, Vec<usize>) = paths
        .par_map(|path| {
            let mut session = shared.session(path);
            let Some(file) = session.load(path) else { return (Vec::new(), 0) };
            let bodies: Vec<_> = session.bodies(file).into_iter().filter_map(|(_, body)| body.ok()).collect();
            let failures = bodies
                .iter()
                .filter_map(|body| body.verify(session.origins).err().map(|malformed| (malformed, body)))
                .map(|(malformed, body)| format!("{}: {malformed}\n{body}", path.display()));
            (failures.collect(), bodies.len())
        })
        .into_iter()
        .unzip();
    let failures: Vec<_> = failures.concat();
    assert!(failures.is_empty(), "{failures:#?}");
    assert!(lowered.iter().sum::<usize>() > 0, "no body lowered");
}

#[test]
fn every_function_a_corpus_program_reaches_is_concrete() {
    let paths = repo_root().join(ACCEPT).files_with(EXTENSION).unwrap();
    let shared = Shared::default();
    let (unready, reached): (Vec<Vec<String>>, Vec<usize>) = paths
        .par_map(|path| {
            let mut session = shared.session(path);
            let Some(file) = session.load(path) else { return (Vec::new(), 0) };
            let program = session.program(file);
            let unready = program.unready.iter().map(|(_, why)| format!("{}: {why}", path.display()));
            (unready.collect(), program.functions.len())
        })
        .into_iter()
        .unzip();
    let unready: Vec<_> = unready.concat();
    assert!(unready.is_empty(), "{unready:#?}");
    assert!(reached.iter().sum::<usize>() > 0, "no corpus program exports anything");
}
