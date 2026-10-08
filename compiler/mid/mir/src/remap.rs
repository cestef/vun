//! Renumbering what a body's statements and terminators name, as expanding it into another body
//! does (spec 03, section 4).

use vun_index::IndexVec;

use crate::place::{Place, Projection};
use crate::{BlockId, Local, Operand, Rvalue, Stmt, TerminatorKind};

/// Where each local and block of one body lives in another.
pub(crate) struct Remap {
    pub(crate) locals: IndexVec<Local, Local>,
    pub(crate) blocks: IndexVec<BlockId, BlockId>,
}

impl Remap {
    pub(crate) fn place(&self, place: &mut Place) {
        place.local = self.locals[place.local];
        for projection in &mut place.projection {
            if let Projection::Index(local) = projection {
                *local = self.locals[*local];
            }
        }
    }

    pub(crate) fn operand(&self, operand: &mut Operand) {
        if let Operand::Place(place) = operand {
            self.place(place);
        }
    }

    pub(crate) fn rvalue(&self, value: &mut Rvalue) {
        for operand in value.operands_mut() {
            self.operand(operand);
        }
        if let Rvalue::Ref { place, .. }
        | Rvalue::IsVariant { place, .. }
        | Rvalue::Resume { frame: place, .. }
        | Rvalue::End { frame: place } = value
        {
            self.place(place);
        }
        for place in value.outputs_mut() {
            self.place(place);
        }
    }

    pub(crate) fn stmt(&self, stmt: &mut Stmt) {
        self.place(&mut stmt.place);
        self.rvalue(&mut stmt.value);
    }

    pub(crate) fn terminator(&self, kind: &mut TerminatorKind) {
        for operand in kind.operands_mut() {
            self.operand(operand);
        }
        match kind {
            TerminatorKind::Goto(to) => *to = self.blocks[*to],
            TerminatorKind::Branch { then, otherwise, .. } => {
                (*then, *otherwise) = (self.blocks[*then], self.blocks[*otherwise]);
            }
            TerminatorKind::Yield { resume, cancel, .. } => {
                (*resume, *cancel) = (self.blocks[*resume], self.blocks[*cancel]);
            }
            TerminatorKind::Defer { consumes, run, skip } => {
                for local in consumes {
                    *local = self.locals[*local];
                }
                (*run, *skip) = (self.blocks[*run], self.blocks[*skip]);
            }
            TerminatorKind::Return | TerminatorKind::Ended | TerminatorKind::Unreachable => {}
        }
    }
}
