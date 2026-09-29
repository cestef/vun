//! The Language Server Protocol as typed messages, read and written through `vun-json`: JSON-RPC
//! framing aside, every value a method exchanges is a struct or enum here, and every method a type
//! naming its parameters and result.

pub mod base;
pub mod document;
pub mod language;
pub mod lifecycle;
pub mod methods;
pub mod rpc;

pub use methods::{Notification, Request, notifications, requests};
