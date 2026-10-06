//! Servers as components (decision 498): a [`Server`] is a program that runs until its client
//! leaves, such as the language server. [`Servers`] is the interface a server library is served
//! through, [`Remote`] the `Server` the host runs it by. The server is the same crate linked in
//! or loaded; the process's standard streams are the client's either way, and it asks nothing of
//! the host.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::io;

use vun_command::Outcome;
use vun_diag::DiagCtxt;

pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Servers;

pub trait Server: Send + Sync {
    /// What `vun` calls it: `lsp`.
    fn name(&self) -> &'static str;

    /// Serves the client over the process's standard streams until it leaves, reporting what
    /// went wrong in `diag`.
    ///
    /// # Errors
    /// Returns why talking to the client failed.
    fn serve(&self, diag: &mut DiagCtxt) -> io::Result<Outcome>;
}
