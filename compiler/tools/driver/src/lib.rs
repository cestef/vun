//! The compiler as a library: sessions, registries, commands.
//!
//! The CLI, the language server and the test harness are thin shells over this crate. Backends
//! and linkers are registered statically through Cargo features; external ones are reached
//! through the `process` adapters.

pub mod checks;
pub mod codegen;
pub mod compare;
pub mod cover;
pub mod coverage;
mod doc;
pub mod editor;
pub mod fix;
pub mod formats;
pub mod install;
pub mod lints;
pub mod packages;
mod pages;
mod plan;
mod positions;
pub mod program;
mod queries;
mod reduce;
mod registry;
mod session;
mod settings;
pub mod setup;
mod shared;
mod shell;
mod stack;
pub mod stages;
mod steps;
pub mod subject;
pub mod target;
pub mod testing;
mod tools;
pub mod trace;
pub mod view;

pub use coverage::Coverage;
pub use pages::Page;
pub use queries::Persistence;
pub use registry::{Codegen, Link, Linkage, Pick, Picked, Registry, Unchosen, Unfit};
pub use session::Session;
pub use shared::Shared;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The commit `cargo xtask install` built this from; none for a plain `cargo build`.
pub const BUILD: Option<&str> = option_env!("VUN_BUILD");
