//! Impls in a file (spec 02, section 3): each provides the requirements of its interface, with
//! types the requirements accept, and no
//! two impls the file sees register a witness for the same interface and type.

use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;

use super::Checker;
use crate::decl::DeclValue;
use crate::member::{Found, MemberDecl, MemberTable, MemberType, MemberValue, Members};
use crate::ty::{Marker, Ty, TyKind, Value};
use crate::witness::{ImplDecl, Impls};

impl Checker<'_, '_, '_> {
    pub(super) fn impls(&mut self) {
        for decl in self.env.cx.get::<Impls>(&self.env.file).iter() {
            self.requirements(decl);
            self.unique(decl);
        }
    }

    /// Reports each requirement of `I(T)` that the witness of `decl` does not provide, or provides
    /// with a type the requirement does not accept. The witness of a type constructor is checked
    /// per type it is used for.
    fn requirements(&mut self, decl: &ImplDecl) {
        let (Value::Type(_), Some(contract)) = (&decl.target, &decl.contract) else { return };
        let cx = self.env.cx;
        let Some(witness) = cx.get::<DeclValue>(&decl.decl).ok().and_then(|v| v.as_type()) else { return };
        let (Some(provided), Some(required)) =
            (cx.get::<Members>(&witness), cx.get::<Members>(&contract.requirements))
        else {
            return;
        };
        for requirement in required.decls.iter().filter(|d| d.decl.required(cx)) {
            let d = if provided.find(requirement.name) == Found::Absent {
                let msg = Msg::new().text("the impl does not provide ").code(requirement.name.as_str());
                let label = Msg::new().code(contract.requirements.to_string()).text(" requires it");
                Diagnostic::error(errors::IMPL_MISSING_REQUIREMENT, msg)
                    .label(requirement.decl.decl.origin, Some(label))
            } else if let Some(msg) = self
                .mistyped(contract.requirements, witness, requirement.name)
                .or_else(|| self.unmarked(contract.requirements, requirement, &provided, witness))
            {
                let label = Msg::new().text("required here");
                Diagnostic::error(errors::IMPL_REQUIREMENT_TYPE, msg)
                    .label(requirement.decl.decl.origin, Some(label))
            } else {
                continue;
            };
            self.diag.error(d.at(decl.attr, None));
        }
    }

    /// Why the witness's `name` does not have a type the requirement `name` of `requirements`
    /// accepts. What a requirement's type names that the witness decides (another requirement,
    /// such as `Error`) does not evaluate there, so only a type that does is compared. Where
    /// `type` is required, a value that is a type is accepted, as a tuple of types is (decision 89).
    fn mistyped(&self, requirements: Ty, witness: Ty, name: Symbol) -> Option<Msg> {
        let cx = self.env.cx;
        let expected = cx.get::<MemberType>(&(requirements, name)).ok()?.ty;
        if expected == TyKind::of_types() {
            let value = cx.get::<MemberValue>(&(witness, name)).ok()?;
            if value.as_type().is_some() {
                return None;
            }
        }
        let found = cx.get::<MemberType>(&(witness, name)).ok()?.ty;
        (!expected.accepts(&found)).then(|| {
            Msg::new()
                .code(name.as_str())
                .text(" is ")
                .ty(found.to_string())
                .text(", but ")
                .code(requirements.to_string())
                .text(" requires ")
                .ty(expected.to_string())
        })
    }

    /// Why the witness's member for `requirement` does not carry the markers the requirement
    /// declares (spec 07, section 4): compared by the markers alone, for a requirement whose type
    /// does not evaluate here, such as one with `$` parameters.
    fn unmarked(
        &self,
        requirements: Ty,
        requirement: &MemberDecl,
        provided: &MemberTable,
        witness: Ty,
    ) -> Option<Msg> {
        let cx = self.env.cx;
        let required = requirement.decl.attributes(cx)?.markers().to_vec();
        let name = requirement.name;
        let found = match cx.get::<MemberType>(&(witness, name)).ok().and_then(|k| k.ty.signature()) {
            Some(signature) => signature.markers,
            None => match provided.find(name) {
                Found::Decl(decl) => decl.decl.attributes(cx)?.markers().to_vec(),
                _ => return None,
            },
        };
        if Marker::accepts(&required, &found) {
            return None;
        }
        let missing: Vec<String> =
            required.iter().filter(|m| !found.contains(m)).map(ToString::to_string).collect();
        Some(
            Msg::new()
                .code(name.as_str())
                .text(" lacks ")
                .code(missing.join(" "))
                .text(", which ")
                .code(requirements.to_string())
                .text(" requires"),
        )
    }

    /// Reports `decl` when an impl before it registers a witness for a pair it covers too.
    fn unique(&mut self, decl: &ImplDecl) {
        let cx = self.env.cx;
        let impls = self.env.impl_files(None).into_iter().flat_map(|f| cx.get::<Impls>(&f).to_vec());
        let Some(first) = impls
            .filter(|other| other.decl < decl.decl && other.overlaps(decl))
            .min_by(|a, b| a.decl.cmp(&b.decl))
        else {
            return;
        };
        let Some(contract) = &decl.contract else { return };
        let msg = Msg::new().text("a second impl of ").code(contract.requirements.to_string());
        let d = Diagnostic::error(errors::IMPL_DUPLICATE, msg)
            .at(decl.attr, None)
            .label(first.attr, Some(Msg::new().text("the first impl is here")));
        self.diag.error(d);
    }
}
