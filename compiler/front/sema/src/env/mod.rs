//! A file's environment: what its names lead to, and the value of its constant expressions,
//! inside a compile-time call when there is one.
//!
//! Forms that need no function body are computed here: literals, names, members of namespaces,
//! `struct` and `enum` literals, ranges, tuples, function types and applications of primitives.
//! Calls go to the installed evaluator (spec 13, section 6). Anything else is [`Unevaluated`],
//! which checks treat as unknown and never report.

mod aggregate;
mod code;
mod condition;
mod eval;
mod path;

pub use condition::Selected;

use std::collections::HashMap;
use std::sync::Arc;

use overt_hir::db::{CheckedNames, FileNames, FileRef, Namespace, Outer, Package, Parsed, ParsedFile, Scope};
use overt_hir::resolve::Res;
use overt_hir::tree::{BinderId, Entry, ExprId, ExprKind, Name, Param, PatKind};
use overt_intern::Symbol;
use overt_origin::OriginTable;
use overt_query::Cx;
use overt_span::{SourceFiles, SourceMap};

use crate::decl::DeclValue;
use crate::eval::{Func, Instance, Staging};
use crate::item::{DeclRef, Declared, ExprRef, Site};
use crate::member::{Found, MemberValue, Members};
use crate::trap::Trap;
use crate::ty::{Ty, Value};
use crate::typeck::TypedBody;

/// Why an expression has no value at compile time here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Unevaluated {
    /// It depends on run-time values, such as locals and parameters.
    Runtime,
    /// It calls a function, which the interpreter evaluates.
    Interpreter,
    /// It depends on itself.
    Cycle,
    /// A name leads nowhere; name resolution reports it.
    Absent,
    /// A declaration provided from outside that is not a primitive: by the linker or an impl.
    Provided,
    /// A trap fired, which the evaluation root reports.
    Trapped(Box<Trap>),
    /// A form not evaluated yet.
    Unsupported,
}

#[derive(Clone)]
pub struct Env<'a, 'db> {
    pub cx: &'a Cx<'db>,
    pub file: FileRef,
    pub parsed: Arc<ParsedFile>,
    names: Arc<CheckedNames>,
    /// The compile-time call evaluated in, if any: types built here are identified by it.
    pub instance: Option<Instance>,
    /// The parameters of that call, bound to its arguments.
    bindings: HashMap<BinderId, Value>,
    /// The `$for` bindings evaluated under, also among `bindings`.
    staged: Vec<Staging>,
    /// The types of the expressions evaluated, where typing found them: forms typed by their
    /// context, such as `.{ }` and `.name`, evaluate only with them.
    types: Option<Arc<TypedBody>>,
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
            bindings: HashMap::new(),
            staged: Vec::new(),
            types: None,
        })
    }

    /// The environment inside the compile-time call `instance`: its function's parameters are
    /// bound to its arguments, for the body and for whatever the body builds, such as the field
    /// types of a `struct` it returns.
    #[must_use]
    pub fn within(cx: &'a Cx<'db>, instance: &Instance) -> Option<Self> {
        let mut env = Self::inside(cx, &instance.f, instance.args.iter().cloned().map(Some))?;
        env.instance = Some(instance.clone());
        Some(env)
    }

    /// The environment of the function value `f`'s literal, with its parameters bound to `args`
    /// by position, those given: a signature whose types name `$` parameters is typed here.
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
        let bound: Vec<_> = binders.zip(args).filter_map(|(b, arg)| Some((b?, arg?))).collect();
        env.bindings.extend(bound);
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
    pub fn typed_by(mut self, types: Option<Arc<TypedBody>>) -> Self {
        self.types = types;
        self
    }

    /// The type typing found for `expr`, if any.
    #[must_use]
    pub fn type_of(&self, expr: ExprId) -> Option<Ty> {
        self.types.as_ref()?.types.get(&expr).map(|k| k.ty)
    }

    /// The environment the literal `literal` was evaluated in: inside `instance`, or in its file.
    #[must_use]
    pub fn defining(cx: &'a Cx<'db>, literal: &ExprRef, instance: Option<&Instance>) -> Option<Self> {
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
                let decl = self.parsed.hir.member_decl(b).ok_or(Unevaluated::Runtime)?;
                Ok(Site::Decl(DeclRef::new(self.file.clone(), &self.parsed.hir, decl)))
            }
            Some(Res::Outer { name, ns }) => {
                let spread =
                    self.parsed.resolution.enclosing(ns).flatten().find_map(|n| {
                        self.names.spreads.brought.get(&n).and_then(|brought| brought.get(&name))
                    });
                match spread {
                    Some(ns) => self.cx.get::<Declared>(&(ns.clone(), name)).ok_or(Unevaluated::Absent),
                    None => self.outer(name),
                }
            }
            None => Err(Unevaluated::Unsupported),
        }
    }

    /// Where `name` leads outside the file: the package, a dependency, or the prelude.
    fn outer(&self, name: Symbol) -> Result<Site, Unevaluated> {
        let root = |package| Ok(Site::Namespace(Namespace { package, path: Vec::new() }));
        match self.cx.get::<Scope>(&self.file.package).names.get(&name) {
            Some(Outer::Prelude(ns)) => {
                self.cx.get::<Declared>(&(ns.clone(), name)).ok_or(Unevaluated::Absent)
            }
            Some(Outer::Package(package)) => root(*package),
            Some(Outer::Dep(index)) => {
                let dep = &self.cx.get::<Package>(&self.file.package).deps[*index];
                root(dep.package.clone().map_err(|_| Unevaluated::Absent)?)
            }
            None => Err(Unevaluated::Absent),
        }
    }

    /// The member `name` of `value` at compile time: a member of a namespace, a declaration of a
    /// type's namespace or of its impls, or a variant of an enum type without payload.
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
                    Found::Decl(_) => self.cx.get::<MemberValue>(&(ty, name)),
                    Found::Variant(v) if v.payload.is_empty() => {
                        Ok(Value::Variant { ty, name, payload: Vec::new() })
                    }
                    // Then the type's impls (spec 01, section 4).
                    found @ (Found::Absent | Found::Unknown) => match self.method(ty, name) {
                        Some(method) => self.cx.get::<MemberValue>(&(method.owner, name)),
                        None if found == Found::Absent => Err(Unevaluated::Absent),
                        None => Err(Unevaluated::Unsupported),
                    },
                    Found::Field(_) | Found::Variant(_) => Err(Unevaluated::Unsupported),
                }
            }
            // A field of a struct value.
            Value::Struct { fields, .. } => {
                fields.into_iter().find(|(n, _)| *n == name).map(|(_, v)| v).ok_or(Unevaluated::Absent)
            }
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The value of `site`.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn value_of(&self, site: Site) -> Result<Value, Unevaluated> {
        match site {
            Site::Decl(decl) => self.cx.get::<DeclValue>(&decl),
            Site::Namespace(ns) => Ok(Value::Namespace(ns)),
        }
    }

    /// The source text of `expr`, as written.
    #[must_use]
    pub fn source(&self, expr: ExprId) -> String {
        let (origins, sources) =
            (self.cx.db().resource::<OriginTable>(), self.cx.db().resource::<SourceFiles>());
        let span = origins.source_span(self.parsed.hir.exprs[expr].origin);
        span.range.of(&sources.file(span.file).text).to_owned()
    }
}
