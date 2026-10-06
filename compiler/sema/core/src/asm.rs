//! What typing asks of the check of a block of instructions (spec 07, section 6): `vun-sema-asm`
//! implements it against the architectures' tables, and the driver installs it.

use std::collections::BTreeMap;

use vun_diag::DiagCtxt;
use vun_hir::tree::{BlockId, ExprId};
use vun_primitives::arch::Arch as Form;
use vun_sema_ty::Ty;

use crate::env::Env;
use crate::installed::Installed;

/// A block as sema checked it: each operand's binding is the expression it binds.
pub type Block = vun_inline::Block<ExprId, ExprId, ExprId>;

/// A block as typing found it.
pub struct Request {
    pub block: BlockId,
    /// The callee of each call of the block that is an instruction.
    pub instructions: Vec<ExprId>,
    /// The operand form of `lang.arch` each other callee of the block calls.
    pub forms: BTreeMap<ExprId, Form>,
    /// The type of each value the block binds to a register, where typing found it.
    pub bound: BTreeMap<ExprId, Ty>,
    /// Whether every output the block binds is a place.
    pub placed: bool,
}

/// What checking a block gave.
pub enum Assembled {
    /// Not checked: no architecture's instructions, or an output that is no place.
    Skipped,
    /// An instruction does not fit, which is reported.
    Rejected,
    Checked(Block),
}

pub trait Assembler: Send + Sync {
    /// Checks the block of `request` in `env`, reporting what does not fit to `diag`.
    fn assemble(&self, env: &Env<'_, '_>, request: &Request, diag: &mut DiagCtxt) -> Assembled;
}

/// The installed assembler, set once by the driver. A block is left unchecked without one.
pub type Assemblers = Installed<dyn Assembler>;
