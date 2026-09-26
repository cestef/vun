//! An or-pattern's alternatives bind the same names (spec 01, section 6); their types are the
//! typer's to compare.

use vun_diag::{Diagnostic, Msg, errors};

use super::walk::Resolver;
use crate::tree::{Hir, Joined, PatId};

impl Resolver<'_, '_> {
    /// Reports each name an alternative binds that the first does not, or lacks that it does.
    pub(super) fn alternatives(&mut self, hir: &Hir, alternatives: &[PatId]) {
        for joined in hir.joined(alternatives) {
            let (msg, at) = match joined {
                Joined::Paired { .. } => continue,
                Joined::Extra { other } => {
                    let b = &hir.binders[other];
                    (
                        Msg::new().code(b.name.as_str()).text(" is not bound by the first alternative"),
                        b.origin,
                    )
                }
                Joined::Missing { first, alternative } => {
                    let name = hir.binders[first].name;
                    (
                        Msg::new().text("this alternative does not bind ").code(name.as_str()),
                        hir.pats[alternative].origin,
                    )
                }
            };
            self.cx.diag.error(Diagnostic::error(errors::BINDING_UNEQUAL_ALTERNATIVES, msg).at(at, None));
        }
    }
}
