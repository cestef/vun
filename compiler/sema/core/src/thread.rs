//! Storage per thread (spec 07, section 2): a type marked `#[thread.local]` gives every top-level
//! binding of it one storage per thread, whose borrows are therefore scoped, never `&static`.

use vun_primitives::Primitive;
use vun_primitives::thread::Thread;
use vun_query::Cx;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Nominal, TyKind};

use crate::attributes::Attributed;
use crate::decl::DeclType;

/// A type or declaration whose top-level bindings may have one storage per thread.
pub trait PerThread {
    /// Whether a top-level binding of it has one storage per thread: its type's literal is marked
    /// `#[thread.local]`.
    #[must_use]
    fn per_thread(&self, cx: &Cx<'_>) -> bool;
}

impl PerThread for Nominal {
    fn per_thread(&self, cx: &Cx<'_>) -> bool {
        self.attributes(cx).is_some_and(|a| a.get(Primitive::Thread(Thread::local)).is_some())
    }
}

impl PerThread for DeclRef {
    fn per_thread(&self, cx: &Cx<'_>) -> bool {
        cx.get::<DeclType>(self)
            .is_ok_and(|known| matches!(&*known.ty, TyKind::Nominal(nominal) if nominal.per_thread(cx)))
    }
}
