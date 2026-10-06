//! The `vun` command: a thin shell over `vun-driver`.

use std::process::ExitCode;

mod commands;

use commands::{CHECK_PATH, PROGRAM};
use vun_driver::Session;

/// The compiler allocates many small, short-lived values from many threads, where mimalloc is
/// faster than the system allocator.
#[cfg(all(feature = "mimalloc", not(target_os = "wasi")))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    vun_trace::Config::from_env().install();
    let mut session = Session::persistent();
    #[cfg(feature = "lsp")]
    session.registry.servers.link(Box::new(vun_component_server_lsp::component()));
    #[cfg(feature = "fmt")]
    session.registry.formatters.link(Box::new(vun_component_formatter_vun::component()));
    #[cfg(feature = "doc")]
    session.registry.documenters.link(Box::new(vun_component_documenter_html::component()));
    #[cfg(feature = "ffi")]
    session.registry.binders.link(Box::new(vun_component_binder_c::component()));
    session.check = PROGRAM.invocation(&CHECK_PATH);
    PROGRAM.main(&mut session)
}
