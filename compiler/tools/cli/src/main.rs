//! The `overt` command: a thin shell over `overt-driver`.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use overt_diag::Color;
use overt_driver::Session;
use overt_driver::commands::PROGRAM;

/// Disables colors when set, per <https://no-color.org>.
const NO_COLOR: &str = "NO_COLOR";

fn main() -> ExitCode {
    overt_trace::init(&overt_trace::Config::from_env());
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut session = Session::new();
    let outcome = overt_command::run(&PROGRAM, &argv, &mut session, &mut io::stdout().lock()).unwrap_or_else(
        |refusal| {
            session.diag.error(*refusal.diagnostic);
            refusal.outcome
        },
    );
    let color = if io::stderr().is_terminal() && std::env::var_os(NO_COLOR).is_none() {
        Color::Always
    } else {
        Color::Never
    };
    session.emit(&mut *(session.format.sink)(Box::new(io::stderr()), color));
    outcome.into()
}
