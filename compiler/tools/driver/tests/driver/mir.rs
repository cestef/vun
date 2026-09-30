//! Every function of the repository that MIR lowers verifies (spec 13, section 4), and every body
//! a corpus program reaches is typed through.

use vun_diag::Diagnostic;
use vun_driver::Shared;
use vun_fs::Tree;
use vun_harness::{ACCEPT, SOURCES, repo_root};
use vun_par::Parallel;
use vun_span::{EXTENSION, SourceMap};

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

/// Typing's verifier on real inputs: every form of every body a corpus program reaches is typed,
/// or typing reports why not; `vun debug dump types` marks a form it gave up on silently.
#[test]
fn typing_gives_up_nowhere_a_corpus_program_reaches() {
    let paths = repo_root().join(ACCEPT).files_with(EXTENSION).unwrap();
    let shared = Shared::default();
    let gaps: Vec<Vec<String>> = paths.par_map(|path| {
        let mut session = shared.session(path);
        let Some(file) = session.load(path) else { return Vec::new() };
        // Where typing gave up, the form, and why.
        let located = |d: &Diagnostic| {
            let primary = d.primary.as_ref();
            let at = primary.map(|l| session.sources.locate(session.origins.source_span(l.origin)));
            let at = at.map_or_else(|| path.display().to_string(), |at| at.to_string());
            let why = primary.and_then(|l| l.msg.as_ref()).map(ToString::to_string).unwrap_or_default();
            format!("{at}: {}: {why}", d.message)
        };
        session.gave_up(file).iter().map(located).collect()
    });
    let gaps = gaps.concat();
    assert!(gaps.is_empty(), "{gaps:#?}");
}
