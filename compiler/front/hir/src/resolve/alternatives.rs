//! An or-pattern's alternatives bind the same names (spec 01, section 6); their types are the
//! typer's to compare.

use vun_diag::{Diagnostic, Msg, errors};

use super::walk::Resolver;
use crate::tree::{Hir, Joined, PatId};

impl Resolver<'_, '_> {
    /// Reports each name an alternative binds that the first does not, or lacks that it does.
    pub(super) fn alternatives(&mut self, hir: &Hir, alternatives: &[PatId]) {
        for joined in hir.joined(alternatives) {
            let (name, d) = match joined {
                Joined::Paired { .. } => continue,
                Joined::Extra { other } => {
                    let b = &hir.binders[other];
                    let msg = Msg::new()
                        .code(b.name.as_str())
                        .text(" is bound here but not by the first alternative");
                    (b.name, Diagnostic::error(errors::BINDING_UNEQUAL_ALTERNATIVES, msg).at(b.origin, None))
                }
                Joined::Missing { first, alternative } => {
                    let b = &hir.binders[first];
                    let msg = Msg::new().text("this alternative does not bind ").code(b.name.as_str());
                    let d = Diagnostic::error(errors::BINDING_UNEQUAL_ALTERNATIVES, msg)
                        .at(hir.pats[alternative].origin, None)
                        .label(b.origin, Some(Msg::new().code(b.name.as_str()).text(" is bound here")));
                    (b.name, d)
                }
            };
            let help = Msg::new().text("bind ").code(name.as_str()).text(" in every alternative, or in none");
            let note = "the code after the pattern reads the same names whichever alternative matched";
            self.cx.diag.error(d.help(help).note(note));
        }
    }
}
