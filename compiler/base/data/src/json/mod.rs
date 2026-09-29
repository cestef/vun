//! JSON as RFC 8259 defines it: a strict reader and a writer, for the formats the toolchain reads
//! from other tools (clang's AST) and writes for them (diagnostics, SARIF, editor configurations).
//!
//! Reading accepts exactly the RFC's grammar: any value at the top, the four whitespace bytes
//! only, no trailing commas, no comments, strings of Unicode scalar values (a lone surrogate
//! escape is an error), numbers kept as their text so that none loses precision. A leading UTF-8
//! byte order mark is ignored, which the RFC allows. Strings without escapes borrow from the
//! input. The reader keeps its own stack; the depth limit bounds what recurses over a value
//! afterwards (dropping, comparing, writing), so a hostile text cannot exhaust the host stack.
//!
//! [`Json`] writes a value: `{}` compactly, `{:#}` with each member and item on its own line.

mod parse;
mod string;
#[cfg(test)]
mod tests;
mod write;

pub use parse::{Error, ErrorKind, Parser, parse};
pub use write::Json;
