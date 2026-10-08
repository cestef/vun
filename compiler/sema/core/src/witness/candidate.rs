//! The impls and derived interfaces that may give a type a witness, each made when asked for.

use vun_codec::{Decode, Encode};
use vun_query::{Cx, Cycle, Query};
use vun_sema_ty::Ty;
use vun_sema_ty::call::{Func, Instance};

use super::{Derive, ImplDecl, Witness};

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
