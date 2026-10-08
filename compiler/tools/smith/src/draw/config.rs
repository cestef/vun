//! How large and how deep programs are made.

/// The limits of a program: what fuzzing the compiler and fuzzing a single backend each want
/// differs, so none is fixed.
#[derive(Clone, Debug)]
pub struct Config {
    /// Functions with parameters, called by the others.
    pub helpers: usize,
    /// Functions without parameters whose values `main` folds in.
    pub roots: usize,
    /// The most parameters of a function.
    pub params: usize,
    /// The most statements in a block.
    pub stmts: usize,
    /// How deep expressions nest.
    pub depth: usize,
    /// How deep blocks nest in blocks.
    pub nesting: usize,
    /// The most parts of a tuple or an array.
    pub parts: usize,
    /// The most iterations of a counted loop.
    pub iterations: u32,
    /// The most expressions a function is made of, past which every expression is a value.
    pub nodes: usize,
    /// The most steps a program is allowed to take, as the model counts them: a loop's body
    /// counts as often as it runs, and a call as the callee's.
    pub steps: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            helpers: 4,
            roots: 3,
            params: 3,
            stmts: 6,
            depth: 4,
            nesting: 3,
            parts: 4,
            iterations: 5,
            nodes: 250,
            steps: 20_000,
        }
    }
}

impl Config {
    /// What each function may spend is `steps`; a program of many functions, each calling others,
    /// spends a multiple of it, which the model's fuel allows.
    const FUEL_FACTOR: u64 = 100;

    /// The steps the model takes before it gives a program up.
    #[must_use]
    pub const fn fuel(&self) -> u64 {
        self.steps.saturating_mul(Self::FUEL_FACTOR)
    }
}
