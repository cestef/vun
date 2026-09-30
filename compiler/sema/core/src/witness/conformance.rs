//! Whether a witness provides what its interface requires (spec 02, section 3): each requirement,
//! with a type the requirement accepts and the markers it declares. An impl whose witness is a
//! type is checked where it is declared; one whose witness is made for a use, by a type
//! constructor or for the values of its `_`, where it is used.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_query::{Cx, Query};
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::{Marker, Ty, TyKind};

use super::Required;
use crate::attributes::Attributed;
use crate::member::{Found, MemberDecl, MemberTable, MemberType, MemberValue, Members};

/// A requirement a witness does not meet.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Unmet {
    pub name: Symbol,
    pub requirement: DeclRef,
    pub why: Shortfall,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Shortfall {
    /// The witness does not provide it.
    Missing,
    /// It provides it with a type the requirement does not accept.
    Mistyped { found: Ty, expected: Ty },
    /// What it provides lacks markers the requirement declares (spec 07, section 4).
    Unmarked(Vec<Marker>),
}

/// The requirements of `requirements`, an interface applied to a type, that the witness does not
/// meet.
pub struct Conformance;

impl Query for Conformance {
    const NAME: &'static str = "conformance";
    type Key = (Ty, Ty);
    type Value = Arc<[Unmet]>;

    fn compute(cx: &Cx<'_>, &(witness, requirements): &Self::Key) -> Self::Value {
        let (Some(provided), Some(required)) =
            (cx.get::<Members>(&witness), cx.get::<Members>(&requirements))
        else {
            return Arc::default();
        };
        let checked = Checked { cx, witness, requirements, provided: &provided };
        required
            .decls
            .iter()
            .filter(|d| d.decl.required(cx))
            .filter_map(|requirement| {
                let why = checked.why(requirement)?;
                Some(Unmet { name: requirement.name, requirement: requirement.decl.clone(), why })
            })
            .collect()
    }
}

/// A witness checked against the requirements of an interface applied to a type.
struct Checked<'a, 'db> {
    cx: &'a Cx<'db>,
    witness: Ty,
    requirements: Ty,
    provided: &'a MemberTable,
}

impl Checked<'_, '_> {
    /// Why the witness does not meet `requirement`, if it does not.
    fn why(&self, requirement: &MemberDecl) -> Option<Shortfall> {
        if self.provided.find(requirement.name) == Found::Absent {
            return Some(Shortfall::Missing);
        }
        self.mistyped(requirement.name).or_else(|| self.unmarked(requirement))
    }

    /// The witness's `name` with a type the requirement `name` does not accept. What a
    /// requirement's type names that the witness decides (another requirement, such as `Error`)
    /// does not evaluate there, so only a type that does is compared. Where `type` is required,
    /// a value that is a type is accepted, as a tuple of types is (decision 89).
    fn mistyped(&self, name: Symbol) -> Option<Shortfall> {
        let expected = self.cx.get::<MemberType>(&(self.requirements, name)).ok()?.ty;
        if expected == TyKind::of_types()
            && self.cx.get::<MemberValue>(&(self.witness, name)).ok()?.as_type().is_some()
        {
            return None;
        }
        let found = self.cx.get::<MemberType>(&(self.witness, name)).ok()?.ty;
        (!expected.accepts(&found)).then_some(Shortfall::Mistyped { found, expected })
    }

    /// The markers `requirement` declares that the witness's member lacks: compared by the
    /// markers alone, for a requirement whose type does not evaluate here, such as one with `$`
    /// parameters.
    fn unmarked(&self, requirement: &MemberDecl) -> Option<Shortfall> {
        let required = requirement.decl.attributes(self.cx)?.markers().to_vec();
        let name = requirement.name;
        let found = match self.cx.get::<MemberType>(&(self.witness, name)).ok().and_then(|k| k.ty.signature())
        {
            Some(signature) => signature.markers,
            None => match self.provided.find(name) {
                Found::Decl(decl) => decl.decl.attributes(self.cx)?.markers().to_vec(),
                _ => return None,
            },
        };
        let missing: Vec<Marker> = required.iter().filter(|m| !found.contains(m)).cloned().collect();
        (!Marker::accepts(&required, &found)).then_some(Shortfall::Unmarked(missing))
    }
}

impl Unmet {
    /// Its report, the requirement of `requirements` labeled; the caller places it.
    #[must_use]
    pub fn diagnostic(&self, requirements: Ty) -> Diagnostic {
        let name = self.name.as_str();
        let (id, msg, label) = match &self.why {
            Shortfall::Missing => (
                errors::IMPL_MISSING_REQUIREMENT,
                Msg::new().text("the impl does not provide ").code(name),
                Msg::new().code(requirements.to_string()).text(" requires it"),
            ),
            Shortfall::Mistyped { found, expected } => (
                errors::IMPL_REQUIREMENT_TYPE,
                Msg::new()
                    .code(name)
                    .text(" is ")
                    .ty(found.to_string())
                    .text(", but ")
                    .code(requirements.to_string())
                    .text(" requires ")
                    .ty(expected.to_string()),
                Msg::new().text("required here"),
            ),
            Shortfall::Unmarked(missing) => {
                let missing: Vec<String> = missing.iter().map(ToString::to_string).collect();
                (
                    errors::IMPL_REQUIREMENT_TYPE,
                    Msg::new()
                        .code(name)
                        .text(" lacks ")
                        .code(missing.join(" "))
                        .text(", which ")
                        .code(requirements.to_string())
                        .text(" requires"),
                    Msg::new().text("required here"),
                )
            }
        };
        Diagnostic::error(id, msg).label(self.requirement.decl.origin, Some(label))
    }
}
