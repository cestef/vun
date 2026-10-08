//! Making a program smaller: each way is a site of the program, a statement to drop, an
//! expression to replace by a value of its type or by a part of itself. What a smaller program
//! still does, the caller tries; the sites a program has are the same for one program, so a
//! search over them can be written as a loop over numbers.

mod sample;
mod sites;

use crate::ir::Program;
use sites::Sites;

impl Program {
    /// How many ways there are to make the program smaller.
    #[must_use]
    pub fn shrinks(&self) -> usize {
        let mut sites = Sites::counting();
        sites.program(&mut self.clone());
        sites.seen()
    }

    /// The program made smaller the `nth` way, as `shrinks` counts them.
    #[must_use]
    pub fn shrunk(&self, nth: usize) -> Option<Self> {
        let mut program = self.clone();
        let mut sites = Sites::picking(nth);
        sites.program(&mut program);
        sites.applied().then_some(program)
    }
}

#[cfg(test)]
mod tests;
