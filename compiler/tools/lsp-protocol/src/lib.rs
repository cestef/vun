//! The Language Server Protocol as typed messages, read and written through `vun-data`: JSON-RPC
//! framing aside, every value a method exchanges is a struct or enum here, and every method a type
//! naming its parameters and result.

pub mod assist;
pub mod base;
pub mod command;
pub mod document;
pub mod edit;
pub mod language;
pub mod lifecycle;
pub mod methods;
pub mod rpc;

pub use methods::{Notification, Request, notifications, requests};
