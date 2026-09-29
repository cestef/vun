//! Exports providing typed link symbols (spec 08, section 1, rule 3): each is accepted where the
//! symbol's declared type is expected, for every declaration of that symbol the file sees.

use vun_diag::{Diagnostic, Msg, errors};
use vun_primitives::link::Link;

use super::Checker;
use crate::decl::DeclType;
use crate::link::Links;

impl Checker<'_, '_, '_> {
    pub(super) fn exports(&mut self) {
        let cx = self.env.cx;
        let links = cx.get::<Links>(&self.env.file);
        let exports = links.iter().filter(|l| l.side == Link::export);
        let seen = self.env.links_seen();
        for export in exports {
            let Ok(found) = cx.get::<DeclType>(&export.decl) else { continue };
            let symbols = seen.iter().filter(|l| l.side == Link::symbol && l.symbol == export.symbol);
            for symbol in symbols {
                let Ok(declared) = cx.get::<DeclType>(&symbol.decl) else { continue };
                if declared.ty.accepts(&found.ty) {
                    continue;
                }
                let msg = Msg::new()
                    .text("this export of ")
                    .code(export.symbol.as_str())
                    .text(" has type ")
                    .code(found.ty.to_string())
                    .text(", and the symbol is declared ")
                    .code(declared.ty.to_string());
                let label = Msg::new().text("declared here");
                let d = Diagnostic::error(errors::LINK_EXPORT_MISMATCH, msg)
                    .at(export.attr, None)
                    .label(symbol.decl.decl.origin, Some(label));
                self.diag.error(d);
            }
        }
    }
}
