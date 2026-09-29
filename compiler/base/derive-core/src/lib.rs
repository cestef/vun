//! What the workspace's derives share, on the compiler's `proc_macro` API alone: the item a derive
//! is on, read from its tokens as far as its name, generics, fields and helper attributes; the
//! shapes its values take; and code built as tokens, never as text.
//!
//! Its functions run only inside a procedural macro, as `proc_macro` does.

extern crate proc_macro;

mod attr;
mod case;
mod casing;
mod depth;
mod given;
mod item;
mod tokens;

pub use attr::{Arg, Attrs};
pub use case::Case;
pub use casing::Casing;
pub use given::{Given, Key, On};
pub use item::{Body, Field, Fields, Generics, Item, Param, ParamKind, Unsupported, Variant};
pub use tokens::Tokens;
