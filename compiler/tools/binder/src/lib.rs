//! Binders as components (decision 498): a [`Binder`] reads a foreign header into Vun
//! declarations (spec 07, section 5), as `vun ffi bind` asks. [`Binders`] is the interface a
//! binder library is served through, [`Remote`] the `Binder` the host calls it by. The binder is
//! the same crate linked in or loaded, and asks nothing of the host.

mod bindings;
mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use vun_codec::{Decode, Encode};

pub use bindings::{Bindings, Bytes, Failure, Missing, Reason, Skipped, Unavailable, Unsupported};
pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Binders;

/// What is bound: a header, as the build compiles it.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Request {
    pub header: PathBuf,
    /// The compile flags of the build, such as `-Iinclude` and `--target`.
    pub args: Vec<String>,
    /// The names the file the bindings are written to sees already, which they avoid.
    pub reserved: Vec<String>,
}

pub trait Binder: Send + Sync {
    /// What `vun` calls it: `c`.
    fn name(&self) -> &'static str;

    /// The bindings of the header.
    ///
    /// # Errors
    /// Returns why nothing could be bound.
    fn bind(&self, request: &Request) -> Result<Bindings, Failure>;
}
