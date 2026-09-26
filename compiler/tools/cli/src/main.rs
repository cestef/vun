//! The `vun` command: a thin shell over `vun-driver`.

use std::process::ExitCode;

use vun_driver::commands::PROGRAM;
use vun_driver::{Persistence, Session};

fn main() -> ExitCode {
    vun_trace::Config::from_env().install();
    let mut session = Session::new();
    session.persistence = Persistence::On;
    session.schemes.install(vun_http::SCHEMES);
    PROGRAM.main(&mut session)
}
