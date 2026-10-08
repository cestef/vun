//! Building a value where it is bound (spec 13, section 3): `a = R; b = a` with `a` a temporary
//! nothing else touches becomes `b = R`, so an array or a record the call or the literal builds
//! is written into the binding's storage, not into a temporary and then copied.

use vun_index::IndexVec;

use crate::{Body, Local, Operand, Rvalue, Stmt};

impl Rvalue {
    /// Whether the value is built into the place it is assigned to, which it must not read.
    fn builds(&self) -> bool {
        match self {
            Self::Aggregate { .. } | Self::Repeat { .. } => true,
            Self::Call { .. } => !self.is_tail_call(),
            _ => false,
        }
    }
}

impl Stmt {
    /// The temporary this moves whole into its own local: `b = a`.
    fn moves(&self) -> Option<Local> {
        match &self.value {
            Rvalue::Use(Operand::Place(from))
                if from.projection.is_empty() && self.place.projection.is_empty() =>
            {
                Some(from.local)
            }
            _ => None,
        }
    }
}

impl Body {
    /// Builds into its binding each value a temporary is built in and moved into right after (see
    /// the module), where the temporary is read and written nowhere else and the binding is
    /// written whole nowhere else; its parts may be, and it may be borrowed, after.
    pub(crate) fn forward(&mut self) {
        if self.suspends() {
            return;
        }
        let (reads, writes) = self.accesses();
        let eligible = |body: &Self, built: &Stmt, moved: &Stmt| {
            let (Some(temp), bound) = (moved.moves(), moved.place.local) else { return false };
            let (temp_decl, bound_decl) = (&body.locals[temp], &body.locals[bound]);
            built.value.builds()
                && built.writes_whole() == Some(temp)
                && temp != bound
                && temp_decl.name.is_none()
                && temp_decl.ty.is_some()
                && temp_decl.ty == bound_decl.ty
                && !temp_decl.role.is_static()
                && !bound_decl.role.is_static()
                && reads[temp] == 1
                && writes[temp] == 1
                && writes[bound] == 1
                && !body.params.contains(&bound)
                && bound != body.ret
                && !built.reads().contains(&bound)
        };
        let forwarded: Vec<_> = self
            .blocks
            .iter()
            .flat_map(|(id, block)| {
                let pairs = block.stmts.windows(2).enumerate();
                pairs.filter(|(_, pair)| eligible(self, &pair[0], &pair[1])).map(move |(i, _)| (id, i))
            })
            .collect();
        for (id, i) in forwarded {
            let stmts = &mut self.blocks[id].stmts;
            stmts[i].place = stmts[i + 1].place.clone();
            stmts[i + 1].value = Rvalue::Dead;
        }
    }

    /// How many statements and terminators read each local, and how many write it whole (a borrow
    /// is a read); the parameters and the result are written, the result read, once more.
    fn accesses(&self) -> (IndexVec<Local, u32>, IndexVec<Local, u32>) {
        let mut reads: IndexVec<Local, u32> = IndexVec::filled(0, self.locals.len());
        let mut writes: IndexVec<Local, u32> = IndexVec::filled(0, self.locals.len());
        for (_, block) in self.blocks.iter() {
            for stmt in block.stmts.iter().filter(|s| !matches!(s.value, Rvalue::Dead)) {
                if let Some(local) = stmt.writes_whole() {
                    writes[local] += 1;
                }
                for local in stmt.reads() {
                    reads[local] += 1;
                }
            }
            for local in block.term.kind.reads() {
                reads[local] += 1;
            }
        }
        for &local in self.params.iter().chain([&self.ret]) {
            writes[local] += 1;
        }
        reads[self.ret] += 1;
        (reads, writes)
    }
}
