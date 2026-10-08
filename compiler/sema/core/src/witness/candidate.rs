//! The impls and derived interfaces that may give a type a witness, each made when asked for.

use vun_codec::{Decode, Encode};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::call::{Func, Instance};
use vun_sema_ty::{Ty, Value};

use super::{Derive, ImplDecl, Witness};
use crate::eval::Call;
use crate::member::{Found, Members};

/// Where a candidate's witness comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Source {
    Impl(ImplDecl),
    Derive(Derive),
}

/// An impl that covers a type, or an interface derived for it. Its witness is made when asked
/// for ([`Resolved`]): finding the interface a type has, or the methods of one, costs the
/// witnesses of the impls asked about, not of every impl that covers the type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Candidate {
    pub ty: Ty,
    pub source: Source,
}

impl Candidate {
    /// The interface it implements, if it implements one.
    #[must_use]
    pub fn interface(&self) -> Option<&Func> {
        match &self.source {
            Source::Impl(decl) => decl.contract.as_ref().map(|c| &c.interface),
            Source::Derive(derive) => Some(&derive.interface),
        }
    }

    /// The interface it implements, made by a constructor with these arguments.
    #[must_use]
    pub fn made(&self) -> Option<Instance> {
        self.interface()?.within.as_deref()?.call()
    }

    /// Whether it implements `interface`.
    #[must_use]
    pub fn implements(&self, interface: &Func) -> bool {
        self.interface() == Some(interface)
    }

    /// Whether its witness may have the member `name`. A derived witness has the members of its
    /// interface, so one whose interface lacks `name` is not made to find out; an impl's has its
    /// body's and its interface's defaults (`ImplDecl::may_have`).
    #[must_use]
    pub fn may_have(&self, cx: &Cx<'_>, name: Symbol) -> bool {
        let derive = match &self.source {
            Source::Impl(decl) => return decl.may_have(cx, name),
            Source::Derive(derive) => derive,
        };
        let call = Instance { f: derive.interface.clone(), args: vec![Value::Type(self.ty)] };
        let Some(requirements) = cx.get::<Call>(&call).ok().and_then(|value| value.as_type()) else {
            return true;
        };
        cx.get::<Members>(&requirements).is_none_or(|members| !matches!(members.find(name), Found::Absent))
    }

    /// Its witness, `None` where the impl does not give one for the type.
    #[must_use]
    pub fn witness(&self, cx: &Cx<'_>) -> Option<Witness> {
        cx.get::<Resolved>(self)
    }
}

/// The witness of a candidate.
pub struct Resolved;

impl Query for Resolved {
    const NAME: &'static str = "resolved";
    type Key = Candidate;
    type Value = Option<Witness>;

    fn compute(cx: &Cx<'_>, candidate: &Self::Key) -> Self::Value {
        match &candidate.source {
            Source::Impl(decl) => Witness::of(cx, decl.clone(), candidate.ty),
            Source::Derive(derive) => Witness::derived(cx, derive, candidate.ty),
        }
    }

    fn cycle(_: &Cx<'_>, _: &Self::Key, _: &Cycle) -> Self::Value {
        None
    }
}
