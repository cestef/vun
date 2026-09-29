//! Lexer, postorder CST arrays, error-recovering parser, and a reader of literal declarations.

// The derives name this crate by its path, here too.
extern crate self as vun_syntax;

pub mod case;
pub mod cst;
pub mod lexer;
pub mod literal;
pub mod make;
pub mod parser;
pub mod tokens;
pub mod visibility;
