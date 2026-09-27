//! The parts of linear locals consumed on their own: each local's deepest linear parts, reached
//! by fields and tuple positions. A read of one moves that part, and the local is consumed once
//! each is (spec 05, section 2).

use std::collections::BTreeMap;

use vun_mir::{Body, Local};
use vun_query::Cx;
use vun_sema::kinds::Kinds;
use vun_sema::member::Members;
use vun_sema::ty::{Ty, TyKind};

use super::states::Key;

pub(super) struct Linear(BTreeMap<Local, Vec<Vec<Key>>>);

impl Linear {
    pub(super) fn find(cx: &Cx<'_>, body: &Body, locals: impl Iterator<Item = Local>) -> Self {
        let parts = locals.filter_map(|local| {
            let mut leaves = Vec::new();
            Self::collect(cx, body.locals[local].ty?, &mut Vec::new(), &mut leaves);
            Some((local, leaves))
        });
        Self(parts.collect())
    }

    /// The paths below `path` of the deepest linear parts of a value of `ty`: `path` itself when
    /// no part of it is linear.
    fn collect(cx: &Cx<'_>, ty: Ty, path: &mut Vec<Key>, leaves: &mut Vec<Vec<Key>>) {
        let linear = |ty: &Ty| cx.get::<Kinds>(ty).is_linear();
        let mut parts = Self::parts(cx, ty).into_iter().filter(|(_, t)| linear(t)).peekable();
        if parts.peek().is_none() {
            leaves.push(path.clone());
        }
        for (key, ty) in parts {
            path.push(key);
            Self::collect(cx, ty, path, leaves);
            path.pop();
        }
    }

    /// The parts of a value of `ty` that a path names without running the body.
    fn parts(cx: &Cx<'_>, ty: Ty) -> Vec<(Key, Ty)> {
        match &*ty {
            TyKind::Tuple(items) => items.iter().enumerate().map(|(at, &t)| (Key::Position(at), t)).collect(),
            TyKind::Record(fields) => fields.iter().map(|&(name, t)| (Key::Field(name), t)).collect(),
            _ => cx.get::<Members>(&ty).map_or_else(Vec::new, |members| {
                let fields = members.fields.iter();
                fields.filter_map(|f| Some((Key::Field(f.name), *f.ty.as_ref().ok()?))).collect()
            }),
        }
    }

    pub(super) fn contains(&self, local: Local) -> bool {
        self.0.contains_key(&local)
    }

    /// Whether the part `path` of `local` is linear: it holds one of the deepest linear parts.
    pub(super) fn holds(&self, local: Local, path: &[Key]) -> bool {
        self.leaves(local).iter().any(|leaf| leaf.starts_with(path))
    }

    pub(super) fn leaves(&self, local: Local) -> &[Vec<Key>] {
        self.0.get(&local).map_or(&[], Vec::as_slice)
    }

    pub(super) fn remove(&mut self, local: Local) -> bool {
        self.0.remove(&local).is_some()
    }

    pub(super) fn locals(&self) -> impl Iterator<Item = Local> {
        self.0.keys().copied()
    }
}
