//! Coverage (spec 11, section 6): under `Coverage::Blocks`, lowering a body counts each of its
//! blocks in `lang.cover.map` as the block starts, at the block's number modulo
//! `lang.cover.size`. The number is the stable hash of the instantiation and the block's
//! position, so a block keeps its count from build to build while its function does not change.

use vun_hash::Stable;
use vun_index::Idx;
use vun_items::lang::cover;
use vun_origin::OriginId;
use vun_primitives::abi::Abi;
use vun_query::Cx;
use vun_sema::cover::{Coverage, CoverageOf};
use vun_sema::env::Env;
use vun_sema::link::Exported;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{DeclRef, Site};
use vun_sema_ty::{Ty, Value};

use super::Lowered;
use crate::Concrete;

/// What counting a block reads of `lang.cover`: the map, its type and its size.
pub struct Map {
    pub decl: DeclRef,
    pub ty: Ty,
    pub size: u64,
}

impl Map {
    /// `lang.cover` as `env` sees it; `None` for a `lang` that lacks it.
    #[must_use]
    pub fn of(env: &Env<'_, '_>) -> Option<Self> {
        let Ok(Site::Decl(decl)) = env.site(cover::map) else { return None };
        let ty = env.site_type(Site::Decl(decl.clone())).ok()?.ty;
        let Ok(Value::Int(size)) = env.item(cover::size) else { return None };
        Some(Self { decl, ty, size: u64::try_from(size.to_u128()?).ok()? })
    }
}

/// A block of a body built covered: the count it makes, and where the code it holds is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// Whether it is the first block, which runs whenever the function is called.
    pub entry: bool,
    /// Its count in `lang.cover.map`.
    pub flag: usize,
    /// The origin of each statement and of the terminator it holds.
    pub origins: Vec<OriginId>,
}

impl Region {
    /// The number of block `index` of the body of `i`, modulo `size`.
    #[must_use]
    pub fn number(i: &Instantiation, index: usize, size: u64) -> u64 {
        Stable::of(&(i, index)) % size
    }

    /// The regions of the body of `i` as built covered, one per block; `None` where the body is
    /// not ready, or is not marked (a naked function).
    #[must_use]
    pub fn of(cx: &Cx<'_>, i: &Instantiation) -> Option<Vec<Self>> {
        if cx.get::<CoverageOf>(&()) == Coverage::Off {
            return None;
        }
        let body = cx.get::<Concrete>(i).ok()?;
        let env = i.env(cx)?;
        let size = Map::of(&env)?.size;
        let regions = body.blocks.iter().map(|(id, block)| {
            let stmts = block.stmts.iter().map(|s| s.origin);
            Self {
                entry: id.index() == 0,
                flag: usize::try_from(Self::number(i, id.index(), size)).unwrap_or_default(),
                origins: stmts.chain([block.term.origin]).collect(),
            }
        });
        (i.f.abi(cx) != Some(Abi::naked)).then(|| regions.collect())
    }
}
