//! Frame layouts (spec 06, section 5): which locals a frame holds depends on where they are live,
//! which the lowering stage knows, so it gives frame types their layouts, installed by the driver
//! as the interpreter is for compile-time calls.

use std::sync::{Arc, OnceLock};

use vun_query::Cx;
use vun_sema_ty::Ty;

use super::Layout;

pub trait Framer: Send + Sync {
    /// The layout of the frames of the frame type `ty`, if they have one.
    fn layout(&self, cx: &Cx<'_>, ty: &Ty) -> Option<Layout>;
}

/// The installed framer: a resource of the query database, set once by the driver.
#[derive(Default)]
pub struct Framers(OnceLock<Arc<dyn Framer>>);

impl Framers {
    /// Installs `framer`, unless one is installed.
    pub fn install(&self, framer: Arc<dyn Framer>) {
        let _ = self.0.set(framer);
    }

    /// The layout of the frame type `ty`, `None` without a framer.
    pub(super) fn layout(cx: &Cx<'_>, ty: Ty) -> Option<Layout> {
        cx.db().resource::<Self>().0.get()?.layout(cx, &ty)
    }
}
