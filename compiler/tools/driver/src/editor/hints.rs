//! Types shown where code does not write them: after the name of each binding without a type,
//! the type its value has. A binding whose value is a function, a struct or an enum shows none,
//! its value saying what it is.

use vun_hir::db::Parsed;
use vun_hir::tree::{ExprKind, PatKind};
use vun_origin::Resolve;
use vun_sema_ty::Ty;
use vun_sema_ty::item::DeclRef;
use vun_span::{ByteRange, FileId};

use crate::Session;

/// A type to show at a position.
pub struct TypeHint {
    /// Where it is shown: after the name.
    pub at: u32,
    pub ty: Ty,
}

impl Session {
    /// The types of the bindings of `file` written without one, whose names end within `range`.
    pub fn type_hints(&mut self, file: FileId, range: ByteRange) -> Vec<TypeHint> {
        let key = self.file_ref(file);
        let Some(parsed) = self.db.get::<Parsed>(&key) else { return Vec::new() };
        let hir = &parsed.hir;
        let mut hints = Vec::new();
        for (id, decl) in hir.decls.iter() {
            let (None, Some(value), PatKind::Bind(_)) = (decl.ty, decl.value, &hir.pats[decl.pat].kind)
            else {
                continue;
            };
            let states = matches!(
                hir.exprs[value].kind,
                ExprKind::Fn { .. } | ExprKind::Closure { .. } | ExprKind::Struct(_) | ExprKind::Enum { .. }
            );
            let name = self.source_span(hir.pats[decl.pat].origin);
            if states || name.file != file || name.range.hi < range.lo || range.hi < name.range.hi {
                continue;
            }
            let known = self.decl_known(&DeclRef::new(key.clone(), hir, id));
            hints.extend(known.map(|known| TypeHint { at: name.range.hi, ty: known.ty }));
        }
        hints.sort_by_key(|h| h.at);
        hints
    }
}
