//! Impls in a file (spec 02, section 3): each provides the requirements of its interface, with
//! types the requirements accept, and no
//! two impls the file sees register a witness for the same interface and type.

use vun_diag::{Diagnostic, Msg, errors};
use vun_sema_ty::Value;

use super::Checker;
use vun_sema::decl::DeclValue;
use vun_sema::witness::{Conformance, ImplDecl, Impls};

impl Checker<'_, '_, '_> {
    pub(super) fn impls(&mut self) {
        for decl in self.env.cx.get::<Impls>(&self.env.file).iter() {
            self.requirements(decl);
            self.unique(decl);
        }
    }

    /// Reports each requirement of `I(T)` that the witness of `decl` does not meet. A witness made
    /// for a use, by a type constructor or for the values of `_`, is checked where it is used.
    fn requirements(&mut self, decl: &ImplDecl) {
        let (Value::Type(_), Some(contract)) = (&decl.target, &decl.contract) else { return };
        let cx = self.env.cx;
        let Some(witness) = cx.get::<DeclValue>(&decl.decl).ok().and_then(|v| v.as_type()) else { return };
        for unmet in cx.get::<Conformance>(&(witness, contract.requirements)).iter() {
            self.diag.error(unmet.diagnostic(contract.requirements).at(decl.attr, None));
        }
    }

    /// Reports `decl` when an impl before it registers a witness for a pair it covers too.
    fn unique(&mut self, decl: &ImplDecl) {
        let Some(first) = self
            .env
            .impls(None)
            .into_iter()
            .filter(|other| other.decl < decl.decl && other.overlaps(decl))
            .min_by(|a, b| a.decl.cmp(&b.decl))
        else {
            return;
        };
        let Some(contract) = &decl.contract else { return };
        let msg = Msg::new().text("a second impl of ").code(contract.requirements.to_string());
        let d = Diagnostic::error(errors::IMPL_DUPLICATE, msg)
            .at(decl.attr, None)
            .label(first.attr, Some(Msg::new().text("the first impl is here")))
            .note("a type has one impl of an interface, so each call finds one method")
            .help("remove one of the impls");
        self.diag.error(d);
    }
}
