//! The language server as a component of the toolchain (decision 498): linked into `vun`, or
//! built with the `export` feature as a library of its `components` directory. It serves from a
//! session of its own, as `vun` makes one.

use std::io;

use vun_command::Outcome;
use vun_diag::DiagCtxt;
use vun_driver::Session;
use vun_server::Server;

/// The server this component serves.
#[must_use]
pub fn component() -> impl Server + 'static {
    Lsp
}

struct Lsp;

impl Server for Lsp {
    fn name(&self) -> &'static str {
        "lsp"
    }

    fn serve(&self, diag: &mut DiagCtxt) -> io::Result<Outcome> {
        let mut session = Session::persistent();
        let outcome = vun_lsp::serve(&mut session);
        diag.absorb(&session.diag);
        outcome
    }
}

#[cfg(feature = "export")]
vun_component::export!(vun_server::Servers, component());
