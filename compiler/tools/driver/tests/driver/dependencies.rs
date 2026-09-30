//! What a program reaches in other packages is checked as its own code is: each file it reaches
//! parsed, each body typed where it is reached.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn a_syntax_error_in_a_dependency_the_program_reaches_is_reported() {
    let fx = Fixture::new("dependency-syntax");
    let lib = Fixture::manifest("lib", 1, "1.0.0");
    fx.write("lib", &lib, "broken := () -> bool { (mut a, mut b) := (false, true); a };\n");
    let mut app = Fixture::manifest("app", 2, "0.1.0");
    app.deps.push(Fixture::local("lib", "../lib"));
    let dir = fx.write("app", &app, "code := () -> bool { lib.broken() };\n");
    let reported = fx.check(&Fixture::file(&dir, &app));
    assert!(
        !reported.is_empty() && reported.iter().all(|id| *id == errors::SYNTAX_EXPECTED_TOKEN),
        "{reported:?}"
    );
}
