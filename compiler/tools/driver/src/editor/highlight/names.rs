//! The class of a name: what its binder declares where it is declared, and, where it is used,
//! what it resolves to: a local or parameter, or a declaration classed by its value.

use std::collections::{HashMap, HashSet};

use vun_hir::db::{FileRef, ParsedFile};
use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, ExprId, ExprKind};
use vun_origin::Resolve;
use vun_sema::decl::DeclValue;
use vun_sema::site::{Declaration, NameSite};
use vun_sema_ty::Value;
use vun_sema_ty::item::{DeclRef, ExprRef, Site};
use vun_span::FileId;
use vun_syntax::tokens::TokenId;

use super::{Class, Modifier, Modifiers};
use crate::Session;

/// Where the names of a file are declared and used.
pub(in crate::editor) struct Names {
    key: FileRef,
    /// Each binder, by where its name starts.
    binders: HashMap<u32, BinderId>,
    /// Each name, member and `.name` expression, by where its name ends.
    uses: HashMap<u32, ExprId>,
    params: HashSet<BinderId>,
}

impl Names {
    pub(in crate::editor) fn of(session: &mut Session, file: FileId, parsed: &ParsedFile) -> Self {
        let key = session.file_ref(file);
        let hir = &parsed.hir;
        let at = |origin| {
            let span = session.source_span(origin);
            (span.file == file).then_some(span.range)
        };
        let binders = hir.binders.iter().filter_map(|(b, binder)| Some((at(binder.origin)?.lo, b))).collect();
        let uses = hir
            .exprs
            .iter()
            .filter(|(_, e)| {
                matches!(e.kind, ExprKind::Name(_) | ExprKind::Member { .. } | ExprKind::DotName(_))
            })
            .filter_map(|(id, e)| Some((at(e.origin)?.hi, id)))
            .collect();
        let mut params = HashSet::new();
        for (_, expr) in hir.exprs.iter() {
            if let ExprKind::Fn { params: list, .. } = &expr.kind {
                let mut found = Vec::new();
                vun_hir::tree::each_entry(list, &mut |p| {
                    if let Some(pat) = p.pat {
                        hir.binders(pat, &mut found);
                    }
                });
                params.extend(found);
            }
        }
        Self { key, binders, uses, params }
    }

    /// The class of the name `t`; `None` for one resolving nowhere.
    pub(super) fn class(
        &self,
        session: &Session,
        parsed: &ParsedFile,
        t: TokenId,
    ) -> Option<(Class, Modifiers)> {
        let range = parsed.tokens.range(t);
        if let Some(&b) = self.binders.get(&range.lo) {
            let (class, modifiers) = self.binder(session, parsed, b);
            return Some((class, modifiers.with(Modifier::Declaration)));
        }
        let &e = self.uses.get(&range.hi)?;
        if let Some(Res::Binder(b)) = parsed.resolution.names[e] {
            return Some(self.binder(session, parsed, b));
        }
        let site = session.db.get::<NameSite>(&ExprRef::new(self.key.clone(), &parsed.hir, e));
        match (site, &parsed.hir.exprs[e].kind) {
            (Some(Declaration::Site(Site::Decl(decl))), _) => Some(Self::declared(session, &decl)),
            (Some(Declaration::Site(Site::Namespace(_))), _) => {
                Some((Class::Namespace, Modifiers::default()))
            }
            (Some(Declaration::Local(_)), _) => Some((Class::Variable, Modifiers::default())),
            (None, ExprKind::Member { .. }) => Some((Class::Property, Modifiers::default())),
            (None, ExprKind::DotName(_)) => Some((Class::Variant, Modifiers::default())),
            (None, _) => None,
        }
    }

    /// What the binder `b` names: a member classed by its declaration's value, a parameter, or a
    /// local.
    pub(in crate::editor) fn binder(
        &self,
        session: &Session,
        parsed: &ParsedFile,
        b: BinderId,
    ) -> (Class, Modifiers) {
        let hir = &parsed.hir;
        let binder = &hir.binders[b];
        let modifiers = Modifiers::default()
            .with_if(Modifier::Mutable, binder.mutable)
            .with_if(Modifier::Comptime, binder.comptime);
        if let Some(decl) = hir.member_decl(b) {
            let (class, own) = Self::declared(session, &DeclRef::new(self.key.clone(), hir, decl));
            return (class, modifiers.union(own));
        }
        let local_fn = hir
            .decl_of(b)
            .and_then(|d| hir.decls[d].value)
            .is_some_and(|v| matches!(hir.exprs[v].kind, ExprKind::Fn { .. } | ExprKind::Closure { .. }));
        let class = if self.params.contains(&b) {
            Class::Parameter
        } else if local_fn {
            Class::Function
        } else {
            Class::Variable
        };
        (class, modifiers)
    }

    /// The class of the declaration `decl`, by the value sema computes for it.
    pub(in crate::editor) fn declared(session: &Session, decl: &DeclRef) -> (Class, Modifiers) {
        let plain = |class| (class, Modifiers::default());
        match session.db.get::<DeclValue>(decl) {
            Ok(Value::Type(_) | Value::Primitive(_)) => plain(Class::Type),
            Ok(Value::Fn(_) | Value::Closure { .. } | Value::Constructor { .. }) => plain(Class::Function),
            Ok(Value::Namespace(_)) => plain(Class::Namespace),
            Ok(Value::Variant { .. }) => plain(Class::Variant),
            Ok(_) => (Class::Variable, Modifiers::default().with(Modifier::Readonly)),
            Err(_) => plain(Class::Variable),
        }
    }
}
