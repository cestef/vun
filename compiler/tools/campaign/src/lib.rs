//! A campaign of programs made by chance (spec 11): each seed makes a program, writes it, and has
//! the session try it through every stage and every backend. What comes of each is typed:
//! the program, how it was made, and whether the compiler agreed with itself.

mod model;
mod reduce;

use std::path::PathBuf;

pub use model::Divergence;
pub use reduce::Outcome;

use vun_driver::Session;
use vun_driver::crash::Catching;
use vun_driver::trial::{Agreed, Finding, Reach, Trial};
use vun_fs::Scratch;
use vun_smith::Entropy;
use vun_smith::draw::{Config, Draw};
use vun_smith::ir::Program;
use vun_smith::lower::Spelling;

/// A session trying programs, in a directory of its own.
pub struct Campaign<'s> {
    session: &'s mut Session,
    catching: Catching,
    scratch: Scratch,
    pub config: Config,
    pub trial: Trial,
}

/// A program, and what the trial of it found.
pub struct Tried {
    pub seed: u64,
    pub program: Program,
    pub spelling: Spelling,
    /// The program as source, which `path` holds.
    pub source: String,
    pub path: PathBuf,
    pub result: Result<Agreed, Finding>,
    /// How what the backends agreed on differs from the model, when they did.
    pub divergence: Option<Divergence>,
}

impl<'s> Campaign<'s> {
    /// A campaign through `session`, over programs of `config`, tried as `trial` says.
    #[must_use]
    pub fn new(session: &'s mut Session, config: Config, trial: Trial) -> Self {
        Self { session, catching: Catching::install(), scratch: Scratch::new("smith"), config, trial }
    }

    /// The program the seed makes, tried.
    pub fn seed(&mut self, seed: u64) -> Tried {
        let mut entropy = Entropy::seeded(seed);
        let spelling = *entropy.pick(Spelling::ALL);
        let program = Draw::program(&mut entropy, &self.config);
        self.tried(seed, spelling, program)
    }

    /// `program`, spelled as `spelling`, written and tried.
    pub fn tried(&mut self, seed: u64, spelling: Spelling, program: Program) -> Tried {
        let source = program.source(spelling);
        let path = self.scratch.write(format!("p{seed}.vun"), &source);
        let file = self.session.open(&path, source.clone());
        let result = self.session.trial(&self.catching, file, &self.trial);
        let ran = result
            .as_ref()
            .ok()
            .filter(|agreed| self.trial.reach == Reach::Run && !agreed.backends.is_empty());
        let divergence =
            ran.and_then(|agreed| Divergence::between(&program, self.config.fuel(), &agreed.stdout));
        Tried { seed, program, spelling, source, path, result, divergence }
    }
}
