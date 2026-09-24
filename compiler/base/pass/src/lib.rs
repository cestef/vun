//! Passes over an intermediate representation, with declared dependencies.
//!
//! MIR and LIR checks and transforms implement [`Pass`]. A [`PassManager`] orders them by their
//! declared requirements, so adding a pass never means editing a hard-coded pipeline.

use vun_diag::DiagCx;
use vun_graph::Requirements;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassResult {
    Unchanged,
    Changed,
}

pub struct PassCx<'a> {
    /// New entities record their origins here, with `Via::Lower`.
    pub report: DiagCx<'a>,
}

/// A check or transform over an IR of type `Ir`.
pub trait Pass<Ir: ?Sized> {
    /// Stable identifier, used in `requires` and in origins.
    fn id(&self) -> &'static str;

    /// Ids of passes that must run before this one.
    fn requires(&self) -> &'static [&'static str] {
        &[]
    }

    fn run(&self, ir: &mut Ir, cx: &mut PassCx<'_>) -> PassResult;
}

/// An error in the declared dependencies of a set of passes, by pass id.
pub type OrderError = vun_graph::OrderError<&'static str>;

/// Orders and runs registered passes.
pub struct PassManager<Ir: ?Sized> {
    passes: Vec<Box<dyn Pass<Ir>>>,
}

impl<Ir: ?Sized> Default for PassManager<Ir> {
    fn default() -> Self {
        Self { passes: Vec::new() }
    }
}

impl<Ir: ?Sized> PassManager<Ir> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, pass: Box<dyn Pass<Ir>>) {
        self.passes.push(pass);
    }

    /// The order in which passes will run: requirements first, otherwise registration order.
    ///
    /// # Errors
    /// Returns an error if a requirement is missing or requirements form a cycle.
    pub fn order(&self) -> Result<Vec<usize>, OrderError> {
        let mut requirements = Requirements::new();
        for pass in &self.passes {
            requirements.add(pass.id(), pass.requires().iter().copied());
        }
        requirements.order()
    }

    /// Runs every pass once, in [`order`](Self::order).
    ///
    /// # Errors
    /// Returns an error if the passes cannot be ordered; no pass runs in that case.
    pub fn run(&self, ir: &mut Ir, cx: &mut PassCx<'_>) -> Result<(), OrderError> {
        for i in self.order()? {
            self.passes[i].run(ir, cx);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
