//! Layouts shown on hover: the whole layout of what is hovered, its parts named by its members.

use std::sync::Arc;

use vun_sema::layout::{Shape, Shapes, Text};
use vun_sema::member::{MemberTable, Members};
use vun_sema_ty::Ty;

use crate::Session;

/// A type's layout, its parts named by its members.
pub struct Laid {
    pub shape: Arc<Shape>,
    pub members: Option<Arc<MemberTable>>,
}

impl Laid {
    #[must_use]
    pub fn text(&self) -> Text<'_> {
        self.shape.text(self.members.as_deref())
    }
}

impl Session {
    /// The layout of `ty`, if it has one.
    #[must_use]
    pub fn laid(&self, ty: &Ty) -> Option<Laid> {
        Some(Laid { shape: self.db.get::<Shapes>(ty)?, members: self.db.get::<Members>(ty) })
    }
}
