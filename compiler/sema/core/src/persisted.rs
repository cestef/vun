//! The queries of checking and evaluation whose memos outlive the process: the target, an input,
//! by its trace, and the rest with their values.

use vun_query::Db;
use vun_sema_ty::item::Declared;

use crate::checks::ChecksOf;
use crate::cover::CoverageOf;
use crate::decl::{DeclType, DeclValue, Declaration, TypedDecl};
use crate::env::{DeclComptime, DeclSignature, ItemSite, PrimitiveDecl};
use crate::eval::{Call, MeteredCall, Signed};
use crate::fuel::Callee;
use crate::kinds::Kinds;
use crate::layout::{Exists, Shapes};
use crate::link::Links;
use crate::member::{MemberType, MemberValue, Members};
use crate::target::{Chosen, TargetOf};
use crate::testing::{Runner, Tests};
use crate::trace::TraceOf;
use crate::typed::{Reported, Typed};
use crate::witness::{Conformance, Derives, Impls, Requirement, Witnesses};

pub const PERSISTED: &[fn(&Db)] = &[
    Db::trace::<TargetOf>,
    Db::trace::<ChecksOf>,
    Db::trace::<CoverageOf>,
    Db::trace::<TraceOf>,
    Db::persist::<Chosen>,
    Db::persist::<Runner>,
    Db::persist::<Tests>,
    Db::persist::<DeclType>,
    Db::persist::<DeclValue>,
    Db::persist::<Declaration>,
    Db::persist::<TypedDecl>,
    Db::persist::<Typed>,
    Db::persist::<Reported>,
    Db::persist::<ItemSite>,
    Db::persist::<DeclComptime>,
    Db::persist::<PrimitiveDecl>,
    Db::persist::<DeclSignature>,
    Db::persist::<Call>,
    Db::persist::<MeteredCall>,
    Db::persist::<Signed>,
    Db::persist::<Callee>,
    Db::persist::<Declared>,
    Db::persist::<Kinds>,
    Db::persist::<Exists>,
    Db::persist::<Shapes>,
    Db::persist::<Links>,
    Db::persist::<Members>,
    Db::persist::<MemberType>,
    Db::persist::<MemberValue>,
    Db::persist::<Derives>,
    Db::persist::<Impls>,
    Db::persist::<Requirement>,
    Db::persist::<Witnesses>,
    Db::persist::<Conformance>,
];
