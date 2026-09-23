//! The `overt` command: a thin shell over `overt-driver`.

use std::io;
use std::process::ExitCode;

use overt_driver::Session;
use overt_driver::commands::PROGRAM;
use overt_render_short::ShortRenderer;

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
    session.emit(&mut ShortRenderer::new(io::stderr()));
    outcome.into()
}
