//! The compiler as a library: sessions, registries, commands.
//!
//! The CLI, the language server and the test harness are thin shells over this crate. Backends
//! and linkers are registered statically through Cargo features; external ones are reached
//! through the `process` adapters.

pub mod commands;
mod registry;

pub use registry::Registry;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
