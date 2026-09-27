//! The requirement a witness's member satisfies: the member of the interface of the impl
//! declaring the witness, of the same name. What is written on the requirement then reaches each
//! function implementing it where a reader asks for it (`Func::requirement`), rather than every
//! attribute holding for its witnesses.

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Query};

use super::Impls;
use crate::decl::DeclValue;
use crate::eval::Func;
use crate::item::{DeclRef, ExprRef};
use crate::member::Members;
use crate::ty::Value;

/// A requirement a function implements, and the type the impl is for.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Implemented {
    pub requirement: DeclRef,
    /// `T` of `#[impl(I(T))]`.
    pub target: Value,
}

/// The requirement the function literal is a witness's member for, if it is one: of an impl, in
/// its file, whose witness is a type rather than made by a constructor.
pub struct Requirement;

impl Query for Requirement {
    const NAME: &'static str = "requirement";
    type Key = ExprRef;
    type Value = Option<Implemented>;

    fn compute(cx: &Cx<'_>, literal: &ExprRef) -> Self::Value {
        let impls = cx.get::<Impls>(&literal.file);
        impls.iter().find_map(|decl| {
            let contract = decl.contract.as_ref()?;
            let Ok(Value::Type(witness)) = cx.get::<DeclValue>(&decl.decl) else { return None };
            let members = cx.get::<Members>(&witness)?;
            let member = members.decls.iter().find(
                |m| matches!(cx.get::<DeclValue>(&m.decl), Ok(Value::Fn(f)) if f.literal == *literal),
            )?;
            let requirements = cx.get::<Members>(&contract.requirements)?;
            let requirement = requirements.decls.iter().find(|r| r.name == member.name)?.decl.clone();
            Some(Implemented { requirement, target: decl.target.clone() })
        })
    }
}

impl Func {
    /// The requirement of an interface this function is the witness's member for, if it is one,
    /// and the type the impl is for.
    #[must_use]
    pub fn requirement(&self, cx: &Cx<'_>) -> Option<Implemented> {
        cx.get::<Requirement>(&self.literal)
    }
}
