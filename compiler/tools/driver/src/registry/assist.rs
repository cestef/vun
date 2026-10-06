//! What compiles for the backends that cannot express everything: the first backend that
//! assists for the target, whichever way it is provided.

use vun_backend::{Assist, Assisted, Backend, Request};
use vun_lir::FuncId;
use vun_target::TargetSpec;

use super::Registry;

/// The assistance of a registry's backends.
pub(super) struct Assisting<'r>(&'r Registry);

impl Registry {
    /// What compiles for the backends that cannot express everything.
    #[must_use]
    pub const fn assist(&self) -> impl Assist + '_ {
        Assisting(self)
    }
}

impl Assisting<'_> {
    fn by(&self, target: &TargetSpec) -> Option<&dyn Assist> {
        self.0.backends().filter_map(Backend::assist).find(|by| by.assists(target))
    }
}

impl Assist for Assisting<'_> {
    fn assists(&self, target: &TargetSpec) -> bool {
        self.by(target).is_some()
    }

    fn compile(&self, request: &Request<'_>, ids: &[FuncId]) -> Assisted {
        let by = self.by(request.target).expect("compiled only where `assists` said so");
        by.compile(request, ids)
    }
}
