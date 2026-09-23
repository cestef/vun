//! Passes over an intermediate representation, with declared dependencies.
//!
//! MIR and LIR checks and transforms implement [`Pass`]. A [`PassManager`] orders them by their
//! declared requirements, so adding a pass never means editing a hard-coded pipeline.

use std::collections::HashMap;

use overt_diag::DiagCtxt;
use overt_origin::OriginTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassResult {
    Unchanged,
    Changed,
}

pub struct PassCx<'a> {
    pub diag: &'a mut DiagCtxt,
    /// New entities record their origins here, with `Via::Lower`.
    pub origins: &'a mut OriginTable,
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

/// An error in the declared dependencies of a set of passes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderError {
    /// A pass requires one that is not registered.
    Missing { pass: &'static str, requires: &'static str },
    /// The requirements form a cycle through this pass.
    Cycle(&'static str),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    New,
    Visiting,
    Done,
}

/// Depth-first traversal of pass requirements.
struct Walk<'a, Ir: ?Sized> {
    passes: &'a [Box<dyn Pass<Ir>>],
    index: HashMap<&'static str, usize>,
    marks: Vec<Mark>,
    order: Vec<usize>,
}

impl<Ir: ?Sized> Walk<'_, Ir> {
    fn visit(&mut self, i: usize) -> Result<(), OrderError> {
        let passes = self.passes;
        let pass = &passes[i];
        match self.marks[i] {
            Mark::Done => return Ok(()),
            Mark::Visiting => return Err(OrderError::Cycle(pass.id())),
            Mark::New => {}
        }
        self.marks[i] = Mark::Visiting;
        for &requires in pass.requires() {
            let j = *self.index.get(requires).ok_or(OrderError::Missing { pass: pass.id(), requires })?;
            self.visit(j)?;
        }
        self.marks[i] = Mark::Done;
        self.order.push(i);
        Ok(())
    }
}

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
        let mut walk = Walk {
            passes: &self.passes,
            index: self.passes.iter().enumerate().map(|(i, p)| (p.id(), i)).collect(),
            marks: vec![Mark::New; self.passes.len()],
            order: Vec::with_capacity(self.passes.len()),
        };
        for i in 0..self.passes.len() {
            walk.visit(i)?;
        }
        Ok(walk.order)
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
mod tests {
    use super::*;

    struct Named(&'static str, &'static [&'static str]);

    impl Pass<Vec<&'static str>> for Named {
        fn id(&self) -> &'static str {
            self.0
        }
        fn requires(&self) -> &'static [&'static str] {
            self.1
        }
        fn run(&self, ir: &mut Vec<&'static str>, _: &mut PassCx<'_>) -> PassResult {
            ir.push(self.0);
            PassResult::Changed
        }
    }

    #[test]
    fn requirements_run_first() {
        let mut pm = PassManager::new();
        pm.register(Box::new(Named("borrows", &["places"])));
        pm.register(Box::new(Named("places", &[])));
        let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
        let mut log = Vec::new();
        pm.run(&mut log, &mut PassCx { diag: &mut diag, origins: &mut origins }).unwrap();
        assert_eq!(log, ["places", "borrows"]);
    }

    #[test]
    fn cycles_are_reported() {
        let mut pm: PassManager<Vec<&'static str>> = PassManager::new();
        pm.register(Box::new(Named("a", &["b"])));
        pm.register(Box::new(Named("b", &["a"])));
        assert!(matches!(pm.order(), Err(OrderError::Cycle(_))));
    }
}
