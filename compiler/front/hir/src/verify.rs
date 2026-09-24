//! The verifier of the HIR: it is a tree, so every node is reached from the root exactly once,
//! and every node has an origin in the table (spec 13, section 4).

use vun_index::{Idx, IndexVec};
use vun_origin::{OriginId, OriginTable};

use crate::tree::{BlockId, DeclId, ExprId, Hir, NsId, PatId, PatKind, Spread};
use crate::visit::{self, Visitor};

/// The arenas of the HIR, for reporting which node is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arena {
    Namespace,
    Decl,
    Block,
    Expr,
    Pat,
    Binder,
}

/// Why a HIR is not a well-formed tree; see [`verify`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    NoRoot,
    /// A node that no path from the root reaches.
    Unreached {
        arena: Arena,
        index: usize,
    },
    /// A node reached twice.
    Shared {
        arena: Arena,
        index: usize,
    },
    /// A node whose origin is not in the table.
    NoOrigin {
        arena: Arena,
        index: usize,
    },
}

impl Arena {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Namespace => "namespace",
            Self::Decl => "declaration",
            Self::Block => "block",
            Self::Expr => "expression",
            Self::Pat => "pattern",
            Self::Binder => "binder",
        }
    }
}

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::NoRoot => f.write_str("the HIR has no root namespace"),
            Self::Unreached { arena, index } => {
                write!(f, "{} {index} is not reached from the root", arena.name())
            }
            Self::Shared { arena, index } => write!(f, "{} {index} is reached twice", arena.name()),
            Self::NoOrigin { arena, index } => write!(f, "{} {index} has no origin", arena.name()),
        }
    }
}

impl std::error::Error for Malformed {}

#[derive(Default)]
struct Visits {
    namespaces: Vec<u32>,
    decls: Vec<u32>,
    blocks: Vec<u32>,
    exprs: Vec<u32>,
    pats: Vec<u32>,
    binders: Vec<u32>,
}

impl Visitor for Visits {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        Self::count(&mut self.namespaces, ns.index());
        visit::walk_namespace(self, hir, ns);
    }
    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        Self::count(&mut self.decls, decl.index());
        visit::walk_decl(self, hir, decl);
    }
    fn block(&mut self, hir: &Hir, block: BlockId) {
        Self::count(&mut self.blocks, block.index());
        visit::walk_block(self, hir, block);
    }
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        Self::count(&mut self.exprs, expr.index());
        visit::walk_expr(self, hir, expr);
    }
    fn spread(&mut self, hir: &Hir, spread: &Spread) {
        for b in spread.select.iter().flatten() {
            Self::count(&mut self.binders, b.index());
        }
        self.expr(hir, spread.path);
    }
    fn pat(&mut self, hir: &Hir, pat: PatId) {
        Self::count(&mut self.pats, pat.index());
        match hir.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) | PatKind::At { binder: b, .. } => {
                Self::count(&mut self.binders, b.index());
            }
            _ => {}
        }
        visit::walk_pat(self, hir, pat);
    }
}

impl Hir {
    /// Checks that this HIR is a tree rooted at its root namespace whose origins are all in `origins`.
    ///
    /// # Errors
    /// Returns the first violation, arena by arena.
    pub fn verify(&self, origins: &OriginTable) -> Result<(), Malformed> {
        let root = self.root.ok_or(Malformed::NoRoot)?;
        let mut visits = Visits {
            namespaces: vec![0; self.namespaces.len()],
            decls: vec![0; self.decls.len()],
            blocks: vec![0; self.blocks.len()],
            exprs: vec![0; self.exprs.len()],
            pats: vec![0; self.pats.len()],
            binders: vec![0; self.binders.len()],
        };
        visits.namespace(self, root);
        let arenas = [
            (Arena::Namespace, &visits.namespaces, Visits::origins(&self.namespaces, |n| n.origin)),
            (Arena::Decl, &visits.decls, Visits::origins(&self.decls, |d| d.origin)),
            (Arena::Block, &visits.blocks, Visits::origins(&self.blocks, |b| b.origin)),
            (Arena::Expr, &visits.exprs, Visits::origins(&self.exprs, |e| e.origin)),
            (Arena::Pat, &visits.pats, Visits::origins(&self.pats, |p| p.origin)),
            (Arena::Binder, &visits.binders, Visits::origins(&self.binders, |b| b.origin)),
        ];
        for (arena, counts, node_origins) in arenas {
            for (index, (&visits, origin)) in counts.iter().zip(node_origins).enumerate() {
                match visits {
                    0 => return Err(Malformed::Unreached { arena, index }),
                    1 => {}
                    _ => return Err(Malformed::Shared { arena, index }),
                }
                if !origins.contains(origin) {
                    return Err(Malformed::NoOrigin { arena, index });
                }
            }
        }
        Ok(())
    }
}

impl Visits {
    /// The origins of the nodes of `arena`, in index order.
    fn origins<I: Idx, T>(arena: &IndexVec<I, T>, origin: impl Fn(&T) -> OriginId) -> Vec<OriginId> {
        arena.iter().map(|(_, node)| origin(node)).collect()
    }

    fn count(counts: &mut [u32], index: usize) {
        if let Some(c) = counts.get_mut(index) {
            *c += 1;
        }
    }
}
