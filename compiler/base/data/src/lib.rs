//! Structured data, apart from any format: values of the JSON data model ([`Value`], [`Object`],
//! numbers kept as their text), typed values converted to and from them ([`ToData`],
//! [`FromData`], derived by `#[derive(ToData, FromData)]`), and the formats they are written in:
//! [`json`] (RFC 8259, read and written), [`toml`] and [`lua`] tables (written). A configuration is one typed
//! value, written in whichever format a tool reads.
//!
//! ```
//! let doc = vun_data::json::parse(r#"{ "loc": { "file": "a.h", "line": 3 } }"#).unwrap();
//! assert_eq!(doc["loc"]["file"].as_str(), Some("a.h"));
//! assert_eq!(doc["loc"]["line"].as_u64(), Some(3));
//! assert!(doc["range"]["begin"].is_null());
//! ```

// The derives name this crate by its path, here too.
extern crate self as vun_data;

mod convert;
pub mod json;
pub mod lua;
mod number;
pub mod toml;
mod value;

pub use convert::{Expected, FromData, Mismatch, Step, ToData};
pub use number::Number;
pub use value::{Object, Value};
pub use vun_data_derive::{FromData, ToData};
