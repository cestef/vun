//! Building and running programs through each backend and target, as the tests of code
//! generation and of the runtimes do (what `vun debug compare` shows).

use vun_driver::Codegen;
use vun_driver::compare::Ran;
use vun_items::{Item, targets};
use vun_sema::target::Choice;
use vun_target::Name;

use crate::common::Fixture;

/// The backends every program here is built through, which must agree.
pub(crate) const BACKENDS: [Name; 2] = [vun_backend_c::INFO.id, vun_backend_native::INFO.id];

/// Builds `source` through `backend` for the machine's target: the program's path, in the
/// fixture holding it.
pub(crate) fn build(name: &str, source: &str, backend: Name) -> (Fixture, std::path::PathBuf) {
    let fixture = Fixture::new(&format!("{name}-{backend}"));
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    let file = session.load(&path).unwrap();
    session.codegen = Codegen::through(backend);
    let program = session.build(file, None);
    let program = program.unwrap_or_else(|_| panic!("{:#?}", session.diag));
    (fixture, program)
}

/// Builds `source` through `backend` and runs it.
pub(crate) fn run(name: &str, source: &str, backend: Name) -> std::process::Output {
    let (_fixture, program) = build(name, source, backend);
    std::process::Command::new(program).output().unwrap()
}

/// The targets every program here runs on besides the machine's own, where the machine runs
/// their programs through the runner its tools file names: arm64 and 32-bit Arm Linux, WASI, and
/// 64-bit and 32-bit Windows.
const CROSS: [Item; 5] = [
    targets::linux_arm64::kernel,
    targets::linux_arm::kernel,
    targets::wasi_wasm32::p1,
    targets::windows_x86_64::msvc,
    targets::windows_x86::msvc,
];

/// Builds `source` through each backend that generates code and runs it, for the machine's
/// target and each of `CROSS` the machine runs programs of: its exit status, the same through
/// each (what `vun debug compare` shows).
pub(crate) fn status(name: &str, source: &str) -> Option<i32> {
    status_on(name, source, &CROSS)
}

/// [`status`] for the machine's target and each of `cross` the machine runs programs of.
pub(crate) fn status_on(name: &str, source: &str, cross: &[Item]) -> Option<i32> {
    let cross = cross.iter().map(|&target| Choice::of(target));
    let choices: Vec<Choice> = std::iter::once(Choice::Host).chain(cross).collect();
    // Each target in a session of its own, all at once.
    let statuses: Vec<(String, Option<i32>)> = std::thread::scope(|scope| {
        let runs: Vec<_> = choices
            .iter()
            .enumerate()
            .map(|(i, choice)| scope.spawn(move || statuses_on(&format!("{name}-{i}"), source, choice)))
            .collect();
        runs.into_iter()
            .flat_map(|t| t.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
            .collect()
    });
    assert!(statuses.windows(2).all(|w| w[0].1 == w[1].1), "the targets disagree: {statuses:?}");
    statuses[0].1
}

/// The exit status of `source` built for `choice` through each backend, by target and backend;
/// none for a target the machine does not run programs of.
fn statuses_on(name: &str, source: &str, choice: &Choice) -> Vec<(String, Option<i32>)> {
    let fixture = Fixture::new(name);
    let path = fixture.root.write("main.vun", source);
    let mut session = fixture.session();
    session.choice = choice.clone();
    let file = session.load(&path).unwrap();
    // Choosing the target equips it with the machine's tools.
    let _ = session.lir(file);
    if *choice != Choice::Host && session.runner().is_empty() {
        return Vec::new();
    }
    let runs = session.run_each(file).unwrap_or_else(|_| panic!("{:#?}", session.diag));
    let target = session.target.clone().and_then(Result::ok).map(|t| t.arch.to_string()).unwrap_or_default();
    let statuses: Vec<_> =
        runs.iter().map(|r| (format!("{target} {}", r.backend), r.output.status.code())).collect();
    assert!(Ran::agree(&runs), "the backends disagree: {statuses:?}");
    statuses
}
