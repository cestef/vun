//! Every function of the repository that MIR lowers verifies (spec 13, section 4).

use overt_driver::Session;
use overt_fs::Tree;
use overt_harness::repo_root;
use overt_span::EXTENSION;

#[test]
fn repository_bodies_lower_to_mir_that_verifies() {
    let root = repo_root();
    let (mut failures, mut lowered) = (Vec::new(), 0);
    for dir in ["lib", "corpus/accept"] {
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
