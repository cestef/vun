//! The verifier of the HIR: it is a tree, so every node is reached from the root exactly once,
//! and every node has an origin in the table (spec 13, section 4).

use overt_index::{Idx, IndexVec};
use overt_origin::{OriginId, OriginTable};

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

fn count(counts: &mut [u32], index: usize) {
    if let Some(c) = counts.get_mut(index) {
        *c += 1;
    }
}

impl Visitor for Visits {
    fn namespace(&mut self, hir: &Hir, ns: NsId) {
        count(&mut self.namespaces, ns.index());
        visit::walk_namespace(self, hir, ns);
    }
    fn decl(&mut self, hir: &Hir, decl: DeclId) {
        count(&mut self.decls, decl.index());
        visit::walk_decl(self, hir, decl);
    }
    fn block(&mut self, hir: &Hir, block: BlockId) {
        count(&mut self.blocks, block.index());
        visit::walk_block(self, hir, block);
    }
    fn expr(&mut self, hir: &Hir, expr: ExprId) {
        count(&mut self.exprs, expr.index());
        visit::walk_expr(self, hir, expr);
    }
    fn spread(&mut self, hir: &Hir, spread: &Spread) {
        for b in spread.select.iter().flatten() {
            count(&mut self.binders, b.index());
        }
        self.expr(hir, spread.path);
    }
    fn pat(&mut self, hir: &Hir, pat: PatId) {
        count(&mut self.pats, pat.index());
        match hir.pats[pat].kind {
            PatKind::Bind(b) | PatKind::Rest(Some(b)) | PatKind::At { binder: b, .. } => {
                count(&mut self.binders, b.index());
            }
            _ => {}
        }
        visit::walk_pat(self, hir, pat);
    }
}

/// Checks that `hir` is a tree rooted at its root namespace whose origins are all in `origins`.
///
/// # Errors
/// Returns the first violation, arena by arena.
pub fn verify(hir: &Hir, origins: &OriginTable) -> Result<(), Malformed> {
    let root = hir.root.ok_or(Malformed::NoRoot)?;
    let mut visits = Visits {
        namespaces: vec![0; hir.namespaces.len()],
        decls: vec![0; hir.decls.len()],
        blocks: vec![0; hir.blocks.len()],
        exprs: vec![0; hir.exprs.len()],
        pats: vec![0; hir.pats.len()],
        binders: vec![0; hir.binders.len()],
    };
    visits.namespace(hir, root);
    let arenas = [
        (Arena::Namespace, &visits.namespaces, origins_of(&hir.namespaces, |n| n.origin)),
        (Arena::Decl, &visits.decls, origins_of(&hir.decls, |d| d.origin)),
        (Arena::Block, &visits.blocks, origins_of(&hir.blocks, |b| b.origin)),
        (Arena::Expr, &visits.exprs, origins_of(&hir.exprs, |e| e.origin)),
        (Arena::Pat, &visits.pats, origins_of(&hir.pats, |p| p.origin)),
        (Arena::Binder, &visits.binders, origins_of(&hir.binders, |b| b.origin)),
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

fn origins_of<I: Idx, T>(arena: &IndexVec<I, T>, origin: impl Fn(&T) -> OriginId) -> Vec<OriginId> {
    arena.iter().map(|(_, node)| origin(node)).collect()
}
