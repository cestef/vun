//! Backends as components (decision 498): [`Backends`] is the interface a backend library is
//! served through, [`Remote`] the `Backend` the host calls it by. A backend is the same crate
//! linked in or loaded; only the transport differs.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

pub use protocol::{Answer, Ask, Call, Emit, Reply};
pub use remote::Remote;
pub use serve::Backends;
