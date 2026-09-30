//! The `vun` command: a thin shell over `vun-driver`.

use std::process::ExitCode;

mod commands;

use commands::{CHECK_PATH, PROGRAM};
use vun_driver::{Persistence, Session};

/// The compiler allocates many small, short-lived values from many threads, where mimalloc is
/// faster than the system allocator.
#[cfg(not(target_os = "wasi"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    vun_trace::Config::from_env().install();
    let mut session = Session::new();
    session.persistence = Persistence::On;
    session.shared = Some(session.store.memos());
    session.schemes.install(vun_http::SCHEMES);
    session.language_server = Some(vun_lsp::serve);
    session.check = PROGRAM.invocation(&CHECK_PATH);
    PROGRAM.main(&mut session)
}
