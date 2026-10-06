//! URL schemes as components (decision 498): [`Schemes`] is the interface a scheme library is
//! served through, [`Remote`] the `Scheme` the host reads URLs by. A scheme is the same crate
//! linked in or loaded; only the transport differs, and a scheme asks nothing of the host.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Schemes;
