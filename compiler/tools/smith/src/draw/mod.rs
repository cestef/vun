//! Making programs: each form of the model has a rule that draws what it needs from the
//! entropy, builds it through the typed constructors, and falls back to a simpler form when the
//! types say no.

mod calls;
mod closure;
mod config;
mod expr;
mod float;
mod function;
mod int;
mod literal;
mod loops;
mod matching;
mod place;
mod scope;
mod stmt;
mod tys;

pub use config::Config;

use crate::entropy::Entropy;
use crate::ir::{Bound, Program};
use scope::Scope;

/// A program being made.
pub struct Draw<'a> {
    entropy: &'a mut Entropy,
    config: &'a Config,
    program: Program,
    scope: Scope,
    /// How many closures the program has.
    closures: usize,
    /// The type parameters of the function being made.
    bounds: Vec<Bound>,
    /// What the function being made gives, which a `?` returns from when it finds no value.
    result: Option<crate::ty::Ty>,
    /// How many expressions the function being made has, as far as made.
    spent: usize,
}

impl<'a> Draw<'a> {
    /// The program the entropy makes under `config`.
    #[must_use]
    pub fn program(entropy: &'a mut Entropy, config: &'a Config) -> Program {
        let mut draw = Self {
            entropy,
            config,
            program: Program::default(),
            scope: Scope::default(),
            closures: 0,
            bounds: Vec::new(),
            result: None,
            spent: 0,
        };
        draw.functions();
        draw.program
    }
}
