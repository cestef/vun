//! Components of the toolchain (spec 12, section 6; decision 498): backends, linkers and any
//! other kind a crate declares an [`Interface`] for, linked into `vun` or loaded from a dynamic
//! library, behind one typed interface.
//!
//! A loaded component is reached through [`Component::call`], one C-ABI entry that takes a
//! request and gives a reply as [`Wire`] messages (the `vun-codec` encoding, handles by
//! content), and the host answers what the component asks back through the same shape
//! ([`Interface::Ask`]). No Rust type, allocation, static or thread crosses it: the library has
//! its own interner, thread pool and allocator, and a buffer is freed by the side that allocated
//! it.
//!
//! This is one of the three places of the toolchain that use `unsafe` (the workspace denies it),
//! with `vun-par`'s pool and `vun-clang`. Each block is the end of a pointer crossing, and the invariants are
//! four, each held where the pointer is made:
//!
//! - a descriptor is read for its layout's number first, so that a library of another layout is
//!   refused before any other field is trusted, and its release must be this one's;
//! - a [`Slice`](abi::Slice) is a borrow of the caller's bytes, used only inside the call, and an
//!   [`Owned`](abi::Owned) is read once and freed by the allocator that made it;
//! - the host's `ask` is a borrow of the closure of the call in progress, and the component
//!   uses it only inside that call, on the thread of the call (a [`Asked`] is not `Sync`);
//! - a name a library gives the host to time an activity under is a string of the library, which
//!   stays mapped, and is read only by the host's table.
//!
//! A panic never unwinds across the boundary: each side catches its own and reports it as a
//! [`Status`](abi::Status). Libraries are never unloaded.
#![allow(unsafe_code, reason = "the boundary of a component: pointers of a call, a library's entry")]

pub mod abi;
mod cause;
mod fault;
mod guest;
mod host;
mod interface;
#[cfg(test)]
mod tests;
mod wire;

pub use abi::Kind;
pub use cause::Cause;
pub use fault::Fault;
pub use guest::answer;
pub use host::Component;
pub use interface::{Asked, Interface};
pub use wire::Wire;

/// The release a component must be of: the toolchain's.
pub const RELEASE: &str = env!("CARGO_PKG_VERSION");
