//! A file's environment: what its names lead to, and the value of its constant expressions.
//!
//! Only forms that need no evaluation of function bodies are computed here: literals, names,
//! members of namespaces, `struct` and `enum` literals, ranges and applications of primitive type
//! constructors. Anything else is [`Unevaluated`], which checks treat as unknown and never report;
//! calls wait for the interpreter (spec 13, section 6).

use std::sync::Arc;

use overt_grammar::TokenKind;
use overt_hir::db::{CheckedNames, FileNames, FileRef, Namespace, Outer, Package, Parsed, ParsedFile, Scope};
use overt_hir::resolve::Res;
use overt_hir::tree::{Arg, Entry, ExprId, ExprKind, Lit, Name};
use overt_origin::{OriginId, OriginTable};
use overt_query::Cx;
use overt_span::{SourceFiles, SourceMap};

use crate::Primitive;
use crate::decl::DeclValue;
use crate::int::{Integer, Interval};
use crate::item::{DeclRef, Declared, Site};
use crate::primitives::lang::Lang;
use crate::ty::{Nominal, NominalKind, Ty, TyKind, Value};

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
    /// A form not evaluated yet.
    Unsupported,
}

pub struct Env<'a, 'db> {
    pub cx: &'a Cx<'db>,
    pub file: FileRef,
    pub parsed: Arc<ParsedFile>,
    names: Arc<CheckedNames>,
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
        })
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

    /// The value of `expr` at compile time.
    ///
    /// # Errors
    /// Returns why it has no value at compile time here.
    pub fn eval(&self, expr: ExprId) -> Result<Value, Unevaluated> {
        let hir = &self.parsed.hir;
        let e = &hir.exprs[expr];
        match &e.kind {
            ExprKind::Lit(Lit::Int(n)) => Ok(Value::Int(Integer::from(*n))),
            ExprKind::Name(_) => self.value_of(self.resolve(expr)?),
            ExprKind::Member { object, name: Name::Plain(name) } => match self.eval(*object)? {
                Value::Namespace(ns) => {
                    self.value_of(self.cx.get::<Declared>(&(ns, *name)).ok_or(Unevaluated::Absent)?)
                }
                _ => Err(Unevaluated::Unsupported),
            },
            ExprKind::Enum(_) => Ok(Self::nominal(e.origin, NominalKind::Enum)),
            ExprKind::Struct(_) => Ok(Self::nominal(e.origin, NominalKind::Struct)),
            ExprKind::Prefix { op: TokenKind::Minus, operand, .. } => match self.eval(*operand)? {
                Value::Int(n) => Ok(Value::Int(-&n)),
                _ => Err(Unevaluated::Unsupported),
            },
            ExprKind::Infix { op: op @ (TokenKind::DotDot | TokenKind::DotDotEq), left, right } => {
                let Value::Int(lo) = self.eval(*left)? else { return Err(Unevaluated::Unsupported) };
                let range = match (right.map(|r| self.eval(r)).transpose()?, op) {
                    (None, _) => Some(Interval::at_least(lo)),
                    (Some(Value::Int(hi)), TokenKind::DotDotEq) => Interval::closed(lo, hi),
                    (Some(Value::Int(hi)), _) => Interval::half_open(lo, &hi),
                    _ => return Err(Unevaluated::Unsupported),
                };
                Ok(Value::Range(range))
            }
            ExprKind::Call { callee, args } => self.call(*callee, args),
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// A call: a primitive type constructor applied to constants, or a call for the interpreter.
    fn call(&self, callee: ExprId, args: &[Entry<Arg>]) -> Result<Value, Unevaluated> {
        let Value::Primitive(Primitive::Lang(Lang::Int)) = self.eval(callee)? else {
            return Err(Unevaluated::Interpreter);
        };
        match args {
            [Entry::One(Arg { name: None, value })] => match self.eval(*value)? {
                Value::Range(Some(range)) => Ok(Value::Type(TyKind::int(&range))),
                _ => Err(Unevaluated::Unsupported),
            },
            _ => Err(Unevaluated::Unsupported),
        }
    }

    /// The type a `struct` or `enum` literal written at `literal` evaluates to, outside any
    /// function, so with no compile-time arguments (decision 28).
    fn nominal(literal: OriginId, kind: NominalKind) -> Value {
        Value::Type(TyKind::Nominal(Nominal { literal, args: Vec::new(), kind }).intern())
    }

    /// The type of `expr`, when it is known without inference: so far, integer literals.
    #[must_use]
    pub fn type_of(&self, expr: ExprId) -> Option<Ty> {
        match &self.parsed.hir.exprs[expr].kind {
            ExprKind::Lit(Lit::Int(n)) => Some(TyKind::int(&Interval::singleton(Integer::from(*n)))),
            _ => None,
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
