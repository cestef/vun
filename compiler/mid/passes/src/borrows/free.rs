//! Borrows in free values (spec 05, section 4): a statement that builds a value whose type holds
//! a scoped value inside a type marked `#[kind.free]`, from operands whose types do not, is where
//! the borrow is stored.

use vun_diag::{Msg, errors};
use vun_mir::{Operand, Rvalue};

use super::Checker;

impl Checker<'_> {
    pub(super) fn free_values(&mut self) {
        let body = self.body;
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                let built = matches!(stmt.value, Rvalue::Call { .. } | Rvalue::Aggregate { .. });
                let from_misplaced = stmt.value.operands().into_iter().any(|o| match o {
                    Operand::Place(p) => self.locals.misplaced.contains(&p.local),
                    Operand::Const(_) => false,
                });
                if built && self.locals.misplaced.contains(&stmt.place.local) && !from_misplaced {
                    let msg = Msg::new()
                        .text("this value holds a borrow inside a type that must stay free of them");
                    let help = Msg::new().text("keep the scoped value in a local or a scoped type, or hold an index or an owned copy");
                    self.report(errors::BORROW_IN_FREE_VALUE, stmt.origin, msg, None, Some(help));
                }
            }
        }
    }
}
