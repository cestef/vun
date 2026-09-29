//! JSON as RFC 8259 defines it: a strict reader and a writer, for the formats the toolchain reads
//! from other tools (clang's AST) and writes for them (diagnostics, SARIF).
//!
//! Reading accepts exactly the RFC's grammar: any value at the top, the four whitespace bytes
//! only, no trailing commas, no comments, strings of Unicode scalar values (a lone surrogate
//! escape is an error), numbers kept as their text so that none loses precision. A leading UTF-8
//! byte order mark is ignored, which the RFC allows. Strings without escapes borrow from the
//! input. The reader keeps its own stack; the depth limit bounds what recurses over a value
//! afterwards (dropping, comparing, writing), so a hostile text cannot exhaust the host stack.
//!
//! ```
//! let doc = vun_json::parse(r#"{ "loc": { "file": "a.h", "line": 3 } }"#).unwrap();
//! assert_eq!(doc["loc"]["file"].as_str(), Some("a.h"));
//! assert_eq!(doc["loc"]["line"].as_u64(), Some(3));
//! assert!(doc["range"]["begin"].is_null());
//! ```

// The derives name this crate by its path, here too.
extern crate self as vun_json;

mod convert;
mod number;
mod parse;
mod string;
#[cfg(test)]
mod tests;
mod value;
mod write;

pub use convert::{Expected, FromJson, Mismatch, Step, ToJson};
pub use number::Number;
pub use parse::{Error, ErrorKind, Parser, parse};
pub use value::{Object, Value};
pub use vun_json_derive::{FromJson, ToJson};
