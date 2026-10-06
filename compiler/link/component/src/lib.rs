//! Linkers as components (decision 498): [`Linkers`] is the interface a linker library is served
//! through, [`Remote`] the `Linker` the host calls it by.

mod protocol;
mod remote;
mod serve;
#[cfg(test)]
mod tests;

pub use protocol::{Call, Reply};
pub use remote::Remote;
pub use serve::Linkers;
