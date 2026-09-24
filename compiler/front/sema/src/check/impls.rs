//! Impls in a file (spec 02, section 3): each provides the requirements of its interface, and no
//! two impls the file sees register a witness for the same interface and type.

use overt_diag::{Diagnostic, Msg, errors};

use super::Checker;
use crate::decl::DeclValue;
use crate::member::{Found, Members};
use crate::ty::Value;
use crate::witness::{ImplDecl, Impls};

impl Checker<'_, '_, '_> {
    pub(super) fn impls(&mut self) {
        for decl in self.env.cx.get::<Impls>(&self.env.file).iter() {
            self.requirements(decl);
            self.unique(decl);
        }
    }

    /// Reports each requirement of `I(T)` that the witness of `decl` does not provide. The
    /// witness of a type constructor is checked per type it is used for.
    fn requirements(&mut self, decl: &ImplDecl) {
        let Value::Type(_) = decl.target else { return };
        let cx = self.env.cx;
        let Some(witness) = cx.get::<DeclValue>(&decl.decl).ok().and_then(|v| v.as_type()) else { return };
        let (Some(provided), Some(required)) =
            (cx.get::<Members>(&witness), cx.get::<Members>(&decl.requirements))
        else {
            return;
        };
        for requirement in required.decls.iter().filter(|d| d.decl.required(cx)) {
            if provided.find(requirement.name) == Found::Absent {
                let msg = Msg::new().text("the impl does not provide ").code(requirement.name.as_str());
                let label = Msg::new().code(decl.requirements.to_string()).text(" requires it");
                let d = Diagnostic::error(errors::IMPL_MISSING_REQUIREMENT, msg)
                    .at(decl.attr, None)
                    .label(requirement.decl.decl.origin, Some(label));
                self.diag.error(d);
            }
        }
    }

    /// Reports `decl` when an impl before it registers a witness for a pair it covers too.
    fn unique(&mut self, decl: &ImplDecl) {
        let cx = self.env.cx;
        let impls = self.env.impl_files().into_iter().flat_map(|f| cx.get::<Impls>(&f).to_vec());
        let Some(first) = impls
            .filter(|other| other.decl < decl.decl && other.overlaps(decl))
            .min_by(|a, b| a.decl.cmp(&b.decl))
        else {
            return;
        };
        let msg = Msg::new().text("a second impl of ").code(decl.requirements.to_string());
        let d = Diagnostic::error(errors::IMPL_DUPLICATE, msg)
            .at(decl.attr, None)
            .label(first.attr, Some(Msg::new().text("the first impl is here")));
        self.diag.error(d);
    }
}
