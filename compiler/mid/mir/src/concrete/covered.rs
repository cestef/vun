//! Coverage (spec 11, section 6): under `Coverage::Blocks`, each block of a body sets its flag in
//! `lang.cover.map` as it starts, a store at the block's number modulo `lang.cover.size`. The
//! number is the stable hash of the instantiation and the block's position, so a block keeps its
//! flag from build to build while its function does not change.

use vun_hash::Stable;
use vun_index::Idx;
use vun_items::lang::{cover, int};
use vun_primitives::abi::Abi;
use vun_sema::cover::Coverage;
use vun_sema::env::Env;
use vun_sema::link::Exported;
use vun_sema_int::Integer;
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::Site;
use vun_sema_ty::{Ty, Value};

use crate::place::{Place, Projection};
use crate::{Body, LocalDecl, Operand, Role, Rvalue, Stmt};

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

impl Body {
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
            let number = Stable::of(&(i, block.index())) % map.size;
            let set = [
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
