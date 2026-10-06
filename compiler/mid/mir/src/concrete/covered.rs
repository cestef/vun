//! Coverage (spec 11, section 6): under `Coverage::Blocks`, each block of a body sets its flag in
//! `lang.cover.map` as it starts, a store at the block's number modulo `lang.cover.size`. The
//! number is the stable hash of the instantiation and the block's position, so a block keeps its
//! flag from build to build while its function does not change.

use vun_hash::Stable;
use vun_index::Idx;
use vun_items::lang::{cover, int};
use vun_origin::OriginId;
use vun_primitives::abi::Abi;
use vun_query::Cx;
use vun_sema::cover::{Coverage, CoverageOf};
use vun_sema::env::Env;
use vun_sema::link::Exported;
use vun_sema_int::Integer;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::Site;
use vun_sema_ty::{Ty, Value};

use super::Lowered;
use crate::place::{Place, Projection};
use crate::{Body, Concrete, LocalDecl, Operand, Role, Rvalue, Stmt};

/// What marking a block reads of `lang.cover`: the map, its type, its size, and the type of an
/// index into it.
struct Map {
    decl: vun_sema_ty::item::DeclRef,
    ty: Ty,
    size: u64,
    index: Ty,
}

impl Map {
    /// `lang.cover` as `env` sees it; `None` for a `lang` that lacks it.
    fn of(env: &Env<'_, '_>) -> Option<Self> {
        let Ok(Site::Decl(decl)) = env.site(cover::map) else { return None };
        let ty = env.site_type(Site::Decl(decl.clone())).ok()?.ty;
        let Ok(Value::Int(size)) = env.item(cover::size) else { return None };
        let index = env.item(int::usize).ok()?.as_type()?;
        Some(Self { decl, ty, size: u64::try_from(size.to_u128()?).ok()?, index })
    }
}

/// A block of a body built covered: the flag it sets, and where the code it holds is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// Whether it is the first block, which runs whenever the function is called.
    pub entry: bool,
    /// Its flag in `lang.cover.map`.
    pub flag: usize,
    /// The origin of each statement and of the terminator it holds, besides the marking.
    pub origins: Vec<OriginId>,
}

impl Region {
    /// The number of block `index` of the body of `i`, modulo `size`.
    fn number(i: &Instantiation, index: usize, size: u64) -> u64 {
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
        let marking = Body::MARKING;
        let regions = body.blocks.iter().map(|(id, block)| {
            let stmts = block.stmts.iter().skip(marking).map(|s| s.origin);
            Self {
                entry: id.index() == 0,
                flag: usize::try_from(Self::number(i, id.index(), size)).unwrap_or_default(),
                origins: stmts.chain([block.term.origin]).collect(),
            }
        });
        (i.f.abi(cx) != Some(Abi::naked)).then(|| regions.collect())
    }
}

impl Body {
    /// How many statements marking a block puts at its start.
    const MARKING: usize = 2;

    /// Under `coverage`, sets the flag of each block of this body, the body of `i`, as it starts;
    /// a naked function holds its assembly alone, and is not marked.
    pub(crate) fn covered(&mut self, env: &Env<'_, '_>, coverage: Coverage, i: &Instantiation) {
        if coverage == Coverage::Off || i.f.abi(env.cx) == Some(Abi::naked) {
            return;
        }
        let Some(map) = Map::of(env) else { return };
        let origin = self.origin;
        let flags = self.locals.push(LocalDecl {
            name: None,
            ty: Some(map.ty),
            role: Role::StaticMut(map.decl),
            origin,
        });
        let at = self.locals.push(LocalDecl { name: None, ty: Some(map.index), role: Role::Held, origin });
        for (block, stmts) in self.blocks.iter_mut().map(|(id, b)| (id, &mut b.stmts)) {
            let number = Region::number(i, block.index(), map.size);
            let set: [Stmt; Self::MARKING] = [
                Stmt {
                    place: Place::of(at),
                    value: Rvalue::Use(Operand::Const(Value::Int(Integer::from(u128::from(number))))),
                    origin,
                },
                Stmt {
                    place: Place::of(flags).project(Projection::Index(at)),
                    value: Rvalue::Use(Operand::Const(Value::Int(Integer::from(1)))),
                    origin,
                },
            ];
            stmts.splice(0..0, set);
        }
    }
}
