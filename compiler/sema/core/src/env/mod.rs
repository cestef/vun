//! A file's environment: what its names lead to, and the value of its constant expressions,
//! inside a compile-time call when there is one.
//!
//! Forms that need no function body are computed here: literals, names, members of namespaces,
//! `struct` and `enum` literals, ranges, tuples, function types and applications of primitives.
//! Calls go to the installed evaluator (spec 13, section 6). Anything else is [`Unevaluated`],
//! which checks treat as unknown and never report.

mod aggregate;
mod attr;
mod call;
mod code;
mod condition;
mod context;
mod eval;
mod frame;
mod generated;
mod lasting;
mod matching;
mod named;
mod operator;
mod passing;
mod path;
mod primitive;
mod reach;

use std::sync::Arc;

pub use code::Quoting;
pub use condition::Selected;
pub use context::{Context, Place};
pub use frame::Extents;
pub use passing::Passed;
pub use path::ItemSite;
pub(crate) use primitive::{PrimitiveComptime, PrimitiveDecl, PrimitiveSignature};
pub use reach::Hop;
use vun_hash::Map;
use vun_hir::db::{CheckedNames, FileNames, FileRef, Parsed, ParsedFile};
use vun_hir::resolve::Res;
use vun_hir::tree::{BinderId, Entry, ExprId, ExprKind, Name, Param, PatKind};
use vun_intern::Symbol;
use vun_origin::OriginTable;
use vun_query::Cx;
use vun_sema_ty::call::{Func, Instantiation, Staging};
use vun_sema_ty::item::{DeclRef, Declared, ExprRef, Site};
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, Value};
use vun_span::{SourceFiles, SourceMap};

use crate::member::{Found, MemberValue, Members};
use crate::typeck;

#[derive(Clone)]
pub struct Env<'a, 'db> {
    pub cx: &'a Cx<'db>,
    pub file: FileRef,
    pub parsed: Arc<ParsedFile>,
    names: Arc<CheckedNames>,
    /// The instantiation evaluated in, if any: types and functions built here are identified by
    /// it.
    pub instance: Option<Instantiation>,
    /// The parameters it binds, bound to their values.
    pub(super) bindings: Map<BinderId, Value>,
    /// The `$for` bindings evaluated under, also among `bindings`.
    staged: Vec<Staging>,
    /// What typing found in the body evaluated in, so far while it is typed: forms typed by
    /// their context, such as `.{ }` and `.name`, and calls whose `$` arguments are inferred
    /// evaluate only with it.
    types: Option<Arc<typeck::Found>>,
}

impl<'a, 'db> Env<'a, 'db> {
    /// The environment of `file`, or `None` for a file the driver did not read.
    #[must_use]
    pub fn of(cx: &'a Cx<'db>, file: &FileRef) -> Option<Self> {
        Some(Self {
            cx,
            file: file.clone(),
            parsed: cx.get::<Parsed>(file)?,
            names: cx.get::<FileNames>(file)?,
            instance: None,
            bindings: Map::default(),
            staged: Vec::new(),
            types: None,
        })
    }

    /// The environment inside `instance`: its function's parameters are bound to their values,
    /// for the body and for whatever the body builds, such as the field types of a `struct` it
    /// returns or a closure it makes.
    #[must_use]
    pub fn within(cx: &'a Cx<'db>, instance: &Instantiation) -> Option<Self> {
        Self::inside(cx, &instance.f, instance.args.iter().cloned())
    }

    /// The environment of the function value `f`'s literal, with its parameters bound to `args`
    /// by position, those given: a signature whose types name `$` parameters is typed here. With
    /// none given, it is the environment `f` was evaluated in.
    #[must_use]
    pub fn inside(cx: &'a Cx<'db>, f: &Func, args: impl IntoIterator<Item = Option<Value>>) -> Option<Self> {
        let mut env = match &f.within {
            Some(outer) => Self::within(cx, outer)?,
            None => Self::of(cx, &f.literal.file)?,
        };
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[f.literal.expr.id].kind else { return None };
        let binders = params.iter().map(|p| match p {
            Entry::One(Param { pat: Some(pat), .. }) => match hir.pats[*pat].kind {
                PatKind::Bind(b) => Some(b),
                _ => None,
            },
            _ => None,
        });
        let args: Vec<_> = args.into_iter().collect();
        env.bindings.extend(binders.zip(&args).filter_map(|(b, arg)| Some((b?, arg.clone()?))));
        let instance = Instantiation::new(f.clone(), args);
        if !instance.args.is_empty() {
            env.instance = Some(instance);
        }
        Some(env.staging(&f.staged))
    }

    /// This environment under the `$for` bindings `staged` as well.
    #[must_use]
    pub fn staging(mut self, staged: &[Staging]) -> Self {
        for s in staged {
            self.bindings.insert(s.binder.id, s.value.clone());
            self.staged.push(s.clone());
        }
        self
    }

    /// The value `binder` is bound to here: a parameter of the call evaluated in, or a `$for`
    /// binder.
    #[must_use]
    pub fn bound(&self, binder: BinderId) -> Option<&Value> {
        self.bindings.get(&binder)
    }

    /// The `$for` bindings evaluated under.
    #[must_use]
    pub fn staged(&self) -> &[Staging] {
        &self.staged
    }

    /// The symbol `name` spells here: a plain name, or `$(e)` for `e` a string known at compile
    /// time (spec 03, section 2).
    #[must_use]
    pub fn name(&self, name: &Name) -> Option<Symbol> {
        match name {
            Name::Plain(name) => Some(*name),
            Name::Computed(expr) => {
                let Ok(Value::Array(bytes)) = self.eval(*expr) else { return None };
                let bytes = bytes
                    .iter()
                    .map(|b| match b {
                        Value::Int(n) => n.to_i128().and_then(|n| u8::try_from(n).ok()),
                        _ => None,
                    })
                    .collect::<Option<Vec<u8>>>()?;
                Some(Symbol::intern(std::str::from_utf8(&bytes).ok()?))
            }
            Name::Index(_) | Name::Hole => None,
        }
    }

    /// This environment, evaluating with the types `types` found.
    #[must_use]
    pub fn typed_by(mut self, types: Option<Arc<typeck::Found>>) -> Self {
        self.types = types;
        self
    }

    /// This environment, evaluating with the types typing found for the steps `path` of the body
    /// it is typed by, each inside the one before: a `$for` step's or a tested `$if` branch's.
    #[must_use]
    pub fn stepping(mut self, path: &[typeck::Step]) -> Self {
        for step in path {
            self.types =
                self.types.and_then(|found| found.steps.get(step).map(|typed| Arc::clone(&typed.found)));
        }
        self
    }

    /// The type typing found for `expr`, if any.
    #[must_use]
    pub fn type_of(&self, expr: ExprId) -> Option<Ty> {
        self.types.as_ref()?.types.get(&expr).map(|k| k.ty)
    }

    /// The type `.name` at `expr` is looked up in, as typing found it.
    #[must_use]
    pub fn context_of(&self, expr: ExprId) -> Option<Ty> {
        self.types.as_ref()?.contexts.get(&expr).copied()
    }

    /// Where the written arguments the call `expr` gathers into its callee's pack start, as
    /// typing found it.
    #[must_use]
    pub fn pack(&self, expr: ExprId) -> Option<usize> {
        self.types.as_ref().and_then(|t| t.packs.get(&expr)).copied()
    }

    /// The `$` arguments the call `expr` leaves out, by parameter, as typing inferred them.
    #[must_use]
    pub fn inferred(&self, expr: ExprId) -> &[(usize, Value)] {
        self.types.as_ref().and_then(|t| t.inferred.get(&expr)).map_or(&[], Vec::as_slice)
    }

    /// The environment the literal `literal` was evaluated in: inside `instance`, or in its file.
    #[must_use]
    pub fn defining(cx: &'a Cx<'db>, literal: &ExprRef, instance: Option<&Instantiation>) -> Option<Self> {
        match instance {
            Some(call) => Self::within(cx, call),
            None => Self::of(cx, &literal.file),
        }
    }

    /// Where the name expression `expr` leads.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn resolve(&self, expr: ExprId) -> Result<Site, Unevaluated> {
        match self.parsed.resolution.names[expr] {
            Some(Res::Binder(b)) => {
                let hir = &self.parsed.hir;
                if let Some(decl) = hir.member_decl(b) {
                    return Ok(Site::Decl(DeclRef::new(self.file.clone(), hir, decl)));
                }
                // A name a spread selects is the member of the namespace it spreads.
                let path = hir.spread_of(b).ok_or_else(|| self.unknown(b))?;
                let Value::Namespace(ns) = self.eval(path)? else { return Err(Unevaluated::Unsupported) };
                self.cx.get::<Declared>(&(ns, hir.binders[b].name)).ok_or(Unevaluated::Absent)
            }
            Some(Res::Outer { name, ns }) => self.outside(name, ns),
            None => match &self.parsed.hir.exprs[expr].kind {
                ExprKind::Member { object, name } => self.member_site(*object, name),
                _ => Err(Unevaluated::Unsupported),
            },
        }
    }

    /// The member `name` of `value` at compile time: a member of a namespace, a declaration of a
    /// type's namespace or of its impls, a variant of an enum type without payload, or the
    /// constructor of one with a payload.
    ///
    /// # Errors
    /// Returns why the member has no value here.
    pub fn member(&self, value: Value, name: Symbol) -> Result<Value, Unevaluated> {
        match value {
            Value::Namespace(ns) => {
                self.value_of(self.cx.get::<Declared>(&(ns, name)).ok_or(Unevaluated::Absent)?)
            }
            Value::Type(ty) => {
                let members = self.cx.get::<Members>(&ty);
                match members.as_deref().map_or(Found::Unknown, |m| m.find(name)) {
                    Found::Decl(_) | Found::Spread(_) => self.cx.get::<MemberValue>(&(ty, name)),
                    Found::Variant(v) if v.payload.is_empty() => {
                        Ok(Value::Variant { ty, name, payload: Vec::new() })
                    }
                    Found::Variant(_) => Ok(Value::Constructor { ty, name }),
                    // Then the type's impls (spec 01, section 4).
                    found @ (Found::Absent | Found::Unknown) => match self.method(ty, name) {
                        Some(method) => self.cx.get::<MemberValue>(&(method.owner, name)),
                        // A parameter's members, and a frame's, are what its interface requires.
                        None if let Some(required) = self.required(ty) => {
                            self.cx.get::<MemberValue>(&(required, name))
                        }
                        None if found == Found::Absent => Err(Unevaluated::Absent),
                        None => Err(Unevaluated::Unsupported),
                    },
                    Found::Field(_) => Err(Unevaluated::Unsupported),
                }
            }
            // A field of a struct value.
            Value::Struct { fields, .. } => {
                fields.into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).ok_or(Unevaluated::Absent)
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The source text of `expr`, as written.
    #[must_use]
    pub fn source(&self, expr: ExprId) -> String {
        let (origins, sources) = (OriginTable::global(), SourceFiles::global());
        let span = origins.source_span(self.parsed.hir.exprs[expr].origin);
        span.range.of(&sources.file(span.file).text).to_owned()
    }
}
