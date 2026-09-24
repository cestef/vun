//! How much of a program the stages that exist understand: the share of function-body
//! expressions sema types and of bodies MIR lowers, the progress measure of roadmap phase 1.

use std::fmt;
use std::ops::AddAssign;

use overt_mir::Mir;
use overt_sema::eval::Instantiation;
use overt_sema::typeck::Typed;
use overt_span::FileId;

use crate::Session;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coverage {
    pub bodies: usize,
    pub lowered: usize,
    pub exprs: usize,
    pub typed: usize,
}

impl Session {
    /// The coverage of the functions of `file`.
    pub fn coverage(&mut self, file: FileId) -> Coverage {
        let mut coverage = Coverage::default();
        for f in self.functions(file) {
            coverage.bodies += 1;
            let f = Instantiation::plain(f);
            coverage.lowered += usize::from(self.db.get::<Mir>(&f).is_ok());
            if let Some(typed) = self.db.get::<Typed>(&f) {
                coverage.exprs += typed.exprs;
                coverage.typed += typed.types.len();
            }
        }
        coverage
    }
}

impl AddAssign for Coverage {
    fn add_assign(&mut self, other: Self) {
        self.bodies += other.bodies;
        self.lowered += other.lowered;
        self.exprs += other.exprs;
        self.typed += other.typed;
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let percent = |part: usize, whole: usize| (part * 100).checked_div(whole).unwrap_or(0);
        write!(
            f,
            "sema types {} of {} function-body expressions ({}%); MIR lowers {} of {} functions ({}%)",
            self.typed,
            self.exprs,
            percent(self.typed, self.exprs),
            self.lowered,
            self.bodies,
            percent(self.lowered, self.bodies),
        )
    }
}
