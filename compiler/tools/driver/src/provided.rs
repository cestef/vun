//! What sema asks of the crates above it, installed in the query database (spec 13, section 9).

use std::sync::Arc;

use vun_interp::Interpreter;
use vun_query::Db;
use vun_sema::asm::Assemblers;
use vun_sema::eval::Evaluators;
use vun_sema::layout::Framers;
use vun_sema::typed::Typists;
use vun_sema_asm::Assembly;
use vun_sema_typeck::Typing;

use crate::Session;

impl Session {
    /// Provides sema the evaluator, the assembler, the typist and the framer in `db`.
    pub(crate) fn provide(db: &Db) {
        db.resource::<Evaluators>().install(Arc::new(Interpreter));
        db.resource::<Assemblers>().install(Arc::new(Assembly));
        db.resource::<Typists>().install(Arc::new(Typing));
        db.resource::<Framers>().install(Arc::new(vun_lower::Framing));
    }
}
