//! The queries of checking and evaluation whose memos outlive the process: the target, an input,
//! by its trace, and the rest with their values.

use vun_query::Db;

use crate::asm::ArchTable;
use crate::check::Checked;
use crate::checks::ChecksOf;
use crate::decl::{DeclType, DeclValue, Declaration, TypedDecl};
use crate::env::{ItemSite, PrimitiveComptime, PrimitiveDecl, PrimitiveSignature};
use crate::eval::{Call, MeteredCall, Signed};
use crate::fuel::Callee;
use crate::item::Declared;
use crate::kinds::Kinds;
use crate::layout::{Exists, Shapes};
use crate::link::Links;
use crate::member::{MemberType, MemberValue, Members};
use crate::plan::Planned;
use crate::target::{Chosen, TargetOf};
use crate::trace::TraceOf;
use crate::typeck::{Reported, Typed};
use crate::witness::{Impls, Requirement, Witnesses};

pub const PERSISTED: &[fn(&Db)] = &[
    Db::trace::<TargetOf>,
    Db::trace::<ChecksOf>,
    Db::trace::<TraceOf>,
    Db::persist::<Chosen>,
    Db::persist::<Checked>,
    Db::persist::<DeclType>,
    Db::persist::<DeclValue>,
    Db::persist::<Declaration>,
    Db::persist::<TypedDecl>,
    Db::persist::<Typed>,
    Db::persist::<Reported>,
    Db::persist::<ItemSite>,
    Db::persist::<PrimitiveComptime>,
    Db::persist::<PrimitiveDecl>,
    Db::persist::<PrimitiveSignature>,
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
    Db::persist::<Planned>,
    Db::persist::<Impls>,
    Db::persist::<Requirement>,
    Db::persist::<Witnesses>,
    Db::persist::<ArchTable>,
];
