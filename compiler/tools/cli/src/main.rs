//! The `vun` command: a thin shell over `vun-driver`.

use std::io::{self, IsTerminal};
use std::process::ExitCode;

use vun_diag::Color;
use vun_driver::Session;
use vun_driver::commands::PROGRAM;

/// Disables colors when set, per <https://no-color.org>.
const NO_COLOR: &str = "NO_COLOR";

fn main() -> ExitCode {
    vun_trace::Config::from_env().install();
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut session = Session::new();
    session.schemes.install(vun_http::SCHEMES);
    let outcome = PROGRAM.run(&argv, &mut session, &mut io::stdout().lock()).unwrap_or_else(|refusal| {
        session.diag.error(*refusal.diagnostic);
        refusal.outcome
    });
    let color = if io::stderr().is_terminal() && std::env::var_os(NO_COLOR).is_none() {
        Color::Always
    } else {
        Color::Never
    };
    session.emit(&mut *(session.format.sink)(Box::new(io::stderr()), color));
    outcome.into()
}
