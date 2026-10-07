//! The nodes lints look at in a file: its declarations, the fields and variants of its types,
//! the local bindings and calls in their values, each with the lints `#[lint.allow]` silences
//! around it.

use vun_intern::Symbol;
use vun_items::{Item, lang};
use vun_origin::OriginId;
use vun_sema_ty::Value;
use vun_sema_ty::trap::Unevaluated;

use super::tables::Binds;
use super::walk::Walk;
use vun_sema::env::Env;
use vun_sema::reflect::Fact;

/// A node of a kind lints look at.
pub(super) enum Kind {
    /// A member of a namespace or a type: its name, what it binds, whether any package sees it,
    /// and whether a doc comment says what it is.
    Decl { name: Symbol, binds: Binds, public: bool, documented: bool },
    /// A local binding or a parameter: its name, whether the code names it, whether it is a
    /// parameter, whether a pattern testing the value it matches binds it, and whether it hides
    /// a constant from outside the file, whether it is mutable, and whether its type holds one
    /// integer.
    Local { name: Symbol, used: bool, param: bool, tests: bool, hides: bool, mutable: bool, pinned: bool },
    /// A call, by the value of what it calls.
    Call { callee: Value },
}

/// A node lints look at: its kind, where it is written, and the lints `#[lint.allow]` silences
/// around it.
pub(super) struct Node {
    pub(super) kind: Kind,
    pub(super) origin: OriginId,
    pub(super) allowed: Vec<Value>,
    /// The declaration, field or variant holding it, where an attribute allowing a lint goes.
    pub(super) declared: OriginId,
}

impl Node {
    /// The nodes of the file `env` reads.
    pub(super) fn of(env: &Env<'_, '_>) -> Vec<Self> {
        Walk::of(env)
    }
}

/// What a node's kind is shown to checks as: its `lang.lint` struct, and the fact of each field.
pub(super) const DECL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Decl::name, |_, n| match &n.kind {
        Kind::Decl { name, .. } => Ok(Value::string(name.as_str())),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::binds, |env, n| match &n.kind {
        Kind::Decl { binds, .. } => env.item(binds.item()),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::origin, |_, n| Ok(Value::Origin(n.origin))),
    (lang::lint_Decl::public, |env, n| match &n.kind {
        Kind::Decl { public, .. } => env.decide(*public),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Decl::documented, |env, n| match &n.kind {
        Kind::Decl { documented, .. } => env.decide(*documented),
        _ => Err(Unevaluated::Unsupported),
    }),
];
pub(super) const LOCAL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Local::name, |_, n| match &n.kind {
        Kind::Local { name, .. } => Ok(Value::string(name.as_str())),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::origin, |_, n| Ok(Value::Origin(n.origin))),
    (lang::lint_Local::used, |env, n| match &n.kind {
        Kind::Local { used, .. } => env.decide(*used),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::param, |env, n| match &n.kind {
        Kind::Local { param, .. } => env.decide(*param),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::tests, |env, n| match &n.kind {
        Kind::Local { tests, .. } => env.decide(*tests),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::hides, |env, n| match &n.kind {
        Kind::Local { hides, .. } => env.decide(*hides),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::mutable, |env, n| match &n.kind {
        Kind::Local { mutable, .. } => env.decide(*mutable),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Local::pinned, |env, n| match &n.kind {
        Kind::Local { pinned, .. } => env.decide(*pinned),
        _ => Err(Unevaluated::Unsupported),
    }),
];
pub(super) const CALL: &[(Item, Fact<Node>)] = &[
    (lang::lint_Call::callee, |_, n| match &n.kind {
        Kind::Call { callee } => Ok(callee.clone()),
        _ => Err(Unevaluated::Unsupported),
    }),
    (lang::lint_Call::origin, |_, n| Ok(Value::Origin(n.origin))),
];
