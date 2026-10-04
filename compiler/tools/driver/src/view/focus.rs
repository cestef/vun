//! What a narrowed view shows: the unit of the file holding a byte.

use vun_hir::db::Parsed;
use vun_hir::tree::DeclId;
use vun_origin::Resolve;
use vun_sema_ty::item::ExprRef;
use vun_span::FileId;

use super::Unit;
use crate::Session;

/// A unit of a file a view is narrowed to.
pub(super) enum Focus {
    Function(ExprRef),
    Declaration(DeclId),
}

impl Focus {
    /// The function, for the views of functions.
    pub(super) const fn expr(&self) -> Option<&ExprRef> {
        match self {
            Self::Function(function) => Some(function),
            Self::Declaration(_) => None,
        }
    }

    /// The declaration, for the views of declarations.
    pub(super) const fn decl(&self) -> Option<DeclId> {
        match self {
            Self::Declaration(decl) => Some(*decl),
            Self::Function(_) => None,
        }
    }
}

impl Session {
    /// Whether `file` has a `unit` holding the byte `offset`.
    pub fn has_unit(&mut self, unit: Unit, file: FileId, offset: u32) -> bool {
        self.unit_at(unit, file, offset).is_some()
    }

    /// The `unit` of `file` holding the byte `offset`.
    pub(super) fn unit_at(&mut self, unit: Unit, file: FileId, offset: u32) -> Option<Focus> {
        match unit {
            Unit::Function => self.function_at(file, offset).map(Focus::Function),
            Unit::Declaration => self.declaration_at(file, offset).map(Focus::Declaration),
        }
    }

    /// The innermost member declaration of `file` whose text holds the byte `offset`.
    pub fn declaration_at(&mut self, file: FileId, offset: u32) -> Option<DeclId> {
        let key = self.file_ref(file);
        let parsed = self.db.get::<Parsed>(&key)?;
        let hir = &parsed.hir;
        let holding = hir.decls.iter().filter_map(|(id, decl)| {
            let mut binders = Vec::new();
            hir.binders(decl.pat, &mut binders);
            binders.first().filter(|&&b| hir.member_decl(b).is_some())?;
            let span = self.source_span(decl.origin);
            let holds = span.file == file && span.range.lo <= offset && offset < span.range.hi;
            holds.then_some((span.range.hi - span.range.lo, id))
        });
        holding.min_by_key(|&(len, _)| len).map(|(_, id)| id)
    }
}
