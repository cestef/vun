//! The compiler as a library: sessions, registries, commands.
//!
//! The CLI, the language server and the test harness are thin shells over this crate. Backends
//! and linkers are registered statically through Cargo features; external ones are reached
//! through the `process` adapters.

pub mod codegen;
pub mod commands;
pub mod compare;
mod coverage;
pub mod formats;
pub mod packages;
mod plan;
mod positions;
pub mod program;
mod queries;
mod registry;
mod session;
mod shared;
mod stack;
pub mod stages;
mod steps;
pub mod target;
mod toolchain;

pub use coverage::Coverage;
pub use queries::Persistence;
pub use registry::{Codegen, Registry, Unchosen, Unfit};
pub use session::Session;
pub use shared::Shared;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
