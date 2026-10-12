//! Binding only what is asked for: the declarations the request names, and what they need, so
//! that a header of thousands of declarations binds as the few a program calls.

use vun_binder::Glob;
use vun_clang::{Cursor, Kind, Type, TypeKind};

use crate::Binder;

impl<'u> Binder<'u> {
    /// Keeps the declarations `only` names, and those they use, in source order.
    pub(crate) fn restrict(&mut self, only: &[Glob]) {
        let named = |decl: &Cursor<'_>| {
            Self::names_of(decl).iter().any(|name| only.iter().any(|glob| glob.matches(name)))
        };
        let mut needed = vec![false; self.decls.len()];
        let mut work: Vec<usize> = (0..self.decls.len()).filter(|&at| named(&self.decls[at])).collect();
        while let Some(at) = work.pop() {
            if std::mem::replace(&mut needed[at], true) {
                continue;
            }
            for used in Self::uses(&self.decls[at]) {
                work.extend(self.position(&used));
            }
        }
        let mut kept = needed.into_iter();
        self.decls.retain(|_| kept.next().unwrap_or(false));
    }

    /// The names a request may name `decl` by: an enum's, and those of its constants.
    fn names_of(decl: &Cursor<'_>) -> Vec<String> {
        let constants = decl.children().into_iter().filter(|c| c.kind() == Kind::EnumConstant);
        std::iter::once(decl.name()).chain(constants.map(|c| c.name())).collect()
    }

    /// The type declarations `decl` is written with.
    fn uses(decl: &Cursor<'u>) -> Vec<Cursor<'u>> {
        let mut found = Vec::new();
        match decl.kind() {
            Kind::Function => {
                decl.params().iter().for_each(|p| Self::declarations(p.ty(), &mut found));
                Self::declarations(decl.result(), &mut found);
            }
            Kind::Var => Self::declarations(decl.ty(), &mut found),
            Kind::Typedef => Self::declarations(decl.underlying(), &mut found),
            Kind::Struct | Kind::Union => decl
                .children()
                .iter()
                .filter(|f| f.kind() == Kind::Field)
                .for_each(|f| Self::declarations(f.ty(), &mut found)),
            _ => {}
        }
        found
    }

    /// The declarations of the records, enums and typedefs `ty` is made of.
    fn declarations(ty: Type<'u>, found: &mut Vec<Cursor<'u>>) {
        match ty.kind() {
            TypeKind::Pointer => Self::declarations(ty.pointee(), found),
            TypeKind::Array | TypeKind::IncompleteArray => Self::declarations(ty.element(), found),
            TypeKind::Function => {
                ty.params().into_iter().for_each(|p| Self::declarations(p, found));
                Self::declarations(ty.result(), found);
            }
            TypeKind::Typedef => {
                found.push(ty.declaration());
                Self::declarations(ty.canonical(), found);
            }
            TypeKind::Record | TypeKind::Enum => found.push(ty.declaration()),
            _ => {}
        }
    }

    /// Where the declaration bound for `decl` is among those to bind.
    fn position(&self, decl: &Cursor<'u>) -> Option<usize> {
        let key = Self::key(decl)?;
        self.decls.iter().position(|d| Self::key(d).as_ref() == Some(&key))
    }
}
