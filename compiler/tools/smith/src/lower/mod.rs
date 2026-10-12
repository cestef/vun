//! Printing the model: each form of a program as the source that says it, through
//! `vun_syntax::make`, so that the text is the parser's reading of what was meant.

mod entry;
mod expr;
mod names;
mod stmt;
mod ty;

use vun_syntax::make::{self, Bind};

use crate::ir::{FnId, Function, Program};

/// How the names of `lang` and `std` are written: as the prelude has them in scope, or by the
/// path they are declared at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, vun_variants::Variants)]
pub enum Spelling {
    Prelude,
    Qualified,
}

/// What the hash `main` prints starts as, and is multiplied by: FNV-1a's.
pub const HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
pub const HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A program printed.
pub struct Lowering<'p> {
    program: &'p Program,
    spelling: Spelling,
}

/// One function being printed: what its locals are named by.
struct Body<'p> {
    lowering: &'p Lowering<'p>,
    function: &'p Function,
}

impl Program {
    /// The source of the program, spelled as `spelling` says.
    #[must_use]
    pub fn source(&self, spelling: Spelling) -> String {
        Lowering::new(self, spelling).file().source()
    }
}

impl<'p> Lowering<'p> {
    #[must_use]
    pub fn new(program: &'p Program, spelling: Spelling) -> Self {
        Self { program, spelling }
    }

    /// The file of the program: its functions, then `main`, which runs the roots.
    #[must_use]
    pub fn file(&self) -> make::File {
        let mut items = vec![Self::start()];
        items.extend(self.program.types.iter().filter_map(|(_, ty)| self.declaration(ty)));
        items.extend(self.program.functions.iter().map(|(id, f)| self.function(id, f)));
        items.push(self.main());
        make::File { doc: Vec::new(), spreads: Vec::new(), items }
    }

    fn function(&self, id: FnId, function: &Function) -> make::Item {
        let body = Body { lowering: self, function };
        Bind::new(&names::function(id), body.lowered()).item()
    }
}
