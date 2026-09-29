//! The names that may be written at a position, each with its class. After `.`, the members of
//! what is before it: a namespace's, or those of the struct or enum its value or type is. Anywhere
//! else, the names in scope: the locals of the function holding the position declared before it,
//! the file's declarations, and what the package's files see from outside them.

use vun_hir::db::{
    Exports, FileRef, Members as NamespaceMembers, Namespace, Outer, Parsed, ParsedFile, Scope,
};
use vun_hir::tree::{ExprId, ExprKind};
use vun_intern::Symbol;
use vun_origin::Resolve;
use vun_sema::decl::DeclValue;
use vun_sema::member::Members;
use vun_sema::site::{Declaration, NameSite};
use vun_sema_ty::item::{Declared, ExprRef, Site};
use vun_sema_ty::{Ty, Value};
use vun_span::FileId;

use super::Class;
use super::highlight::names::Names;
use crate::Session;

/// A name that may be written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Completion {
    pub name: Symbol,
    pub class: Class,
}

impl Session {
    /// The names that may be written at `offset` of `file`, by name.
    pub fn completions(&mut self, file: FileId, offset: u32) -> Vec<Completion> {
        let key = self.file_ref(file);
        let Some(parsed) = self.db.get::<Parsed>(&key) else { return Vec::new() };
        let mut found = match self.member_object(file, &parsed, offset) {
            Some(object) => self.members_of(&ExprRef::new(key, &parsed.hir, object)),
            None => self.in_scope(&key, &parsed, file, offset),
        };
        found.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));
        found.dedup_by_key(|c| c.name);
        found
    }

    /// What is before the `.` of the member being written at `offset`, if one is.
    fn member_object(&self, file: FileId, parsed: &ParsedFile, offset: u32) -> Option<ExprId> {
        let hir = &parsed.hir;
        let span = |id: ExprId| self.source_span(hir.exprs[id].origin);
        hir.exprs
            .iter()
            .filter_map(|(id, e)| match e.kind {
                ExprKind::Member { object, .. } => Some((span(id), object)),
                _ => None,
            })
            .filter(|(member, object)| {
                member.file == file && span(*object).range.hi < offset && offset <= member.range.hi
            })
            .min_by_key(|(member, _)| member.range.len())
            .map(|(_, object)| object)
    }

    /// The members of what `object` is: a namespace, a type, or a value of a type.
    fn members_of(&self, object: &ExprRef) -> Vec<Completion> {
        let site = match self.db.get::<NameSite>(object) {
            Some(Declaration::Site(site)) => Some(site),
            _ => None,
        };
        let value = match &site {
            Some(Site::Namespace(ns)) => Some(Value::Namespace(ns.clone())),
            Some(Site::Decl(decl)) => self.db.get::<DeclValue>(decl).ok(),
            None => None,
        };
        match value {
            Some(Value::Namespace(ns)) => self.namespace_members(&object.file, &ns),
            Some(Value::Type(ty)) => self.type_members(ty),
            _ => self.known(object).map(|known| self.type_members(known.ty)).unwrap_or_default(),
        }
    }

    /// The members of `ns` that `asking` sees: all of them in its own package, else the public.
    fn namespace_members(&self, asking: &FileRef, ns: &Namespace) -> Vec<Completion> {
        let names = if ns.package == asking.package {
            self.db.get::<NamespaceMembers>(ns)
        } else {
            self.db.get::<Exports>(ns)
        };
        names
            .iter()
            .map(|&name| Completion {
                name,
                class: self.site_class(self.db.get::<Declared>(&(ns.clone(), name))),
            })
            .collect()
    }

    /// The fields, declarations and variants of `ty`, or of what it points to.
    fn type_members(&self, ty: Ty) -> Vec<Completion> {
        let table = self.db.get::<Members>(&ty).or_else(|| self.db.get::<Members>(&ty.pointee()?));
        let Some(table) = table else { return Vec::new() };
        let fields = table.fields.iter().map(|f| Completion { name: f.name, class: Class::Property });
        let decls =
            table.decls.iter().map(|d| Completion { name: d.name, class: Names::declared(self, &d.decl).0 });
        let variants = table.variants.iter().map(|v| Completion { name: v.name, class: Class::Variant });
        fields.chain(decls).chain(variants).collect()
    }

    /// The names in scope at `offset` of `file`.
    fn in_scope(&mut self, key: &FileRef, parsed: &ParsedFile, file: FileId, offset: u32) -> Vec<Completion> {
        let names = Names::of(self, file, parsed);
        let hir = &parsed.hir;
        // The innermost function holding the position: its parameters and locals declared before it.
        let function = hir
            .exprs
            .iter()
            .filter(|(_, e)| matches!(e.kind, ExprKind::Fn { .. }))
            .map(|(_, e)| self.source_span(e.origin))
            .filter(|s| s.file == file && s.range.lo <= offset && offset <= s.range.hi)
            .min_by_key(|s| s.range.len());
        let declared = hir.root.map(|root| hir.members(root)).unwrap_or_default();
        let mut found: Vec<Completion> = hir
            .binders
            .iter()
            .filter(|&(b, binder)| {
                let at = self.source_span(binder.origin);
                let local = function.is_some_and(|f| f.range.lo <= at.range.lo && at.range.hi <= offset);
                local || declared.contains(&b)
            })
            .map(|(b, binder)| Completion { name: binder.name, class: names.binder(self, parsed, b).0 })
            .collect();
        let scope = self.db.get::<Scope>(&key.package);
        found.extend(scope.names.iter().map(|(&name, outer)| {
            let class = match outer {
                Outer::Package(_) | Outer::Dep(_) => Class::Namespace,
                Outer::Prelude(ns) => self.site_class(self.db.get::<Declared>(&(ns.clone(), name))),
            };
            Completion { name, class }
        }));
        found
    }

    /// The class of what a member name leads to.
    fn site_class(&self, site: Option<Site>) -> Class {
        match site {
            Some(Site::Decl(decl)) => Names::declared(self, &decl).0,
            Some(Site::Namespace(_)) => Class::Namespace,
            None => Class::Variable,
        }
    }
}
