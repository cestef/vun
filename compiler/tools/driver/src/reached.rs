//! What a program reaches: its functions, their MIR, and the LIR they lower to.

use std::sync::Arc;

use vun_lower::{Lowered, Unlowered};
use vun_mir::{Body, Mir, Program, Programs, Roots, Unbuilt};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::ExprRef;
use vun_span::FileId;

use crate::Session;

impl Session {
    /// The program whose root is `file`: its roots and the functions they reach.
    pub fn program(&mut self, file: FileId) -> Arc<Program> {
        let key = self.file_ref(file);
        self.db.get::<Programs>(&(key, self.roots))
    }

    /// The LIR of the program whose root is `file`, or why it does not lower.
    pub fn lir(&mut self, file: FileId) -> Arc<Result<vun_lir::Unit, Unlowered>> {
        let key = self.file_ref(file);
        self.db.get::<Lowered>(&(key, self.roots))
    }

    /// The MIR of each function literal of `file`, in source order.
    pub fn bodies(&mut self, file: FileId) -> Vec<(ExprRef, Result<Arc<Body>, Unbuilt>)> {
        let functions = self.functions(file);
        functions.into_iter().map(|f| (f.clone(), self.db.get::<Mir>(&Instantiation::plain(f)))).collect()
    }

    /// Whether the program of `file` instantiates the function literal, as one with `$` parameters
    /// is: its bodies are those of its instantiations, not one of its own.
    pub fn is_generic(&mut self, file: FileId, function: &ExprRef) -> bool {
        let key = self.file_ref(file);
        let program = self.db.get::<Programs>(&(key, Roots::Functions));
        program.instantiations().any(|i| i.f.literal == *function && !i.is_plain())
    }
}
