//! Storage per thread (spec 07, section 2): a type marked `#[thread.local]` gives every top-level
//! binding of it one storage per thread, whose borrows are therefore scoped, never `&static`.

use vun_query::Cx;

use crate::Primitive;
use crate::decl::DeclType;
use crate::item::DeclRef;
use crate::primitives::thread::Thread;
use crate::ty::{Nominal, TyKind};

impl Nominal {
    /// Whether a top-level binding of this type has one storage per thread: its literal is
    /// marked `#[thread.local]`.
    #[must_use]
    pub fn per_thread(&self, cx: &Cx<'_>) -> bool {
        self.attributes(cx).is_some_and(|a| a.get(Primitive::Thread(Thread::local)).is_some())
    }
}

impl DeclRef {
    /// Whether the declaration is a top-level binding with one storage per thread.
    #[must_use]
    pub fn per_thread(&self, cx: &Cx<'_>) -> bool {
        cx.get::<DeclType>(self)
            .is_ok_and(|known| matches!(&*known.ty, TyKind::Nominal(nominal) if nominal.per_thread(cx)))
    }
}
