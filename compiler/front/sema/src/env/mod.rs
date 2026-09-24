//! A file's environment: what its names lead to, and the value of its constant expressions,
//! inside a compile-time call when there is one.
//!
//! Forms that need no function body are computed here: literals, names, members of namespaces,
//! `struct` and `enum` literals, ranges, tuples, function types and applications of primitives.
//! Calls go to the installed evaluator (spec 13, section 6). Anything else is [`Unevaluated`],
//! which checks treat as unknown and never report.

mod eval;

use std::collections::HashMap;
use std::sync::Arc;

use overt_hir::db::{CheckedNames, FileNames, FileRef, Namespace, Outer, Package, Parsed, ParsedFile, Scope};
use overt_hir::resolve::Res;
use overt_hir::tree::{BinderId, Entry, ExprId, ExprKind, Param, PatKind};
use overt_origin::OriginTable;
use overt_query::Cx;
use overt_span::{SourceFiles, SourceMap};

use crate::decl::DeclValue;
use crate::eval::Instance;
use crate::item::{DeclRef, Declared, Site};
use crate::ty::Value;

/// Why an expression has no value at compile time here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    /// A trap fired: a division by zero, or a shift past any width.
    Trapped,
    /// A form not evaluated yet.
    Unsupported,
}

pub struct Env<'a, 'db> {
    pub cx: &'a Cx<'db>,
    pub file: FileRef,
    pub parsed: Arc<ParsedFile>,
    names: Arc<CheckedNames>,
    /// The compile-time call evaluated in, if any: types built here are identified by it.
    pub instance: Option<Instance>,
    /// The parameters of that call, bound to its arguments.
    bindings: HashMap<BinderId, Value>,
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
        })
    }

    /// The environment inside the compile-time call `instance`: its function's parameters are
    /// bound to its arguments, for the body and for whatever the body builds, such as the field
    /// types of a `struct` it returns.
    #[must_use]
    pub fn within(cx: &'a Cx<'db>, instance: &Instance) -> Option<Self> {
        let mut env = Self::of(cx, &instance.f.file)?;
        let hir = &env.parsed.hir;
        let ExprKind::Fn { params, .. } = &hir.exprs[instance.f.expr].kind else { return None };
        let binders = params.iter().map(|p| match p {
            Entry::One(Param { pat: Some(pat), .. }) => match hir.pats[*pat].kind {
                PatKind::Bind(b) => Some(b),
                _ => None,
            },
            _ => None,
        });
        let bindings = binders.zip(&instance.args).filter_map(|(b, arg)| Some((b?, arg.clone()))).collect();
        env.bindings = bindings;
        env.instance = Some(instance.clone());
        Some(env)
    }

    /// Where the name expression `expr` leads.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn resolve(&self, expr: ExprId) -> Result<Site, Unevaluated> {
        match self.parsed.resolution.names[expr] {
            Some(Res::Binder(b)) => {
                let decl = self.parsed.hir.member_decl(b).ok_or(Unevaluated::Runtime)?;
                Ok(Site::Decl(DeclRef { file: self.file.clone(), decl }))
            }
            Some(Res::Outer { name, ns }) => {
                let spread =
                    self.parsed.resolution.enclosing(ns).flatten().find_map(|n| {
                        self.names.spreads.brought.get(&n).and_then(|brought| brought.get(&name))
                    });
                let target = match spread {
                    Some(ns) => ns.clone(),
                    None => match self.cx.get::<Scope>(&self.file.package).names.get(&name) {
                        Some(Outer::Prelude(ns)) => ns.clone(),
                        Some(Outer::Package(package)) => {
                            return Ok(Site::Namespace(Namespace { package: *package, path: Vec::new() }));
                        }
                        Some(Outer::Dep(index)) => {
                            let dep = &self.cx.get::<Package>(&self.file.package).deps[*index];
                            let package = dep.package.clone().map_err(|_| Unevaluated::Absent)?;
                            return Ok(Site::Namespace(Namespace { package, path: Vec::new() }));
                        }
                        None => return Err(Unevaluated::Absent),
                    },
                };
                self.cx.get::<Declared>(&(target, name)).ok_or(Unevaluated::Absent)
            }
            None => Err(Unevaluated::Unsupported),
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
