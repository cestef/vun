//! A program (spec 08, section 1): the functions its root file's exports reach. The exports are
//! the `#[link.export]` functions and data among the root file's bindings and the members of the
//! struct types they evaluate to; from each, every function a body calls or takes as a value, or
//! an exported datum holds, is reached, with the `$` arguments of its call.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hash::{Map, Set};
use vun_hir::db::{FileRef, Parsed};
use vun_hir::tree::ExprKind;
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_sema::decl::{DeclType, DeclValue};
use vun_sema::eval::Instantiate;
use vun_sema::link::{Exported, Placed};
use vun_sema::member::{MemberType, MemberValue, Members};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::{DeclRef, ExprRef};
use vun_sema_ty::{Ty, TyKind, Value};
use vun_trace::tracing;

use crate::{Aggregate, Concrete, Lowered, Operand, Rvalue, Unready};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Program {
    /// Every function reached, in the order found: the exports first.
    pub functions: Vec<Instantiation>,
    /// The exported functions, by their position in `functions`, and the symbol of each.
    pub exports: Vec<(usize, Symbol)>,
    /// The exported data, which no function makes: a vector table.
    pub data: Vec<ExportedDatum>,
    /// The declarations the linker provides that a body reaches.
    pub imports: Vec<DeclRef>,
    /// The functions reached whose bodies are not ready, and why.
    pub unready: Vec<(Instantiation, Unready)>,
    /// The variants whose constructors are taken as values, each a function (`Body::constructor`),
    /// by its enum type and name.
    pub constructors: Vec<(Ty, Symbol)>,
}

/// A declaration whose value is exported as data.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ExportedDatum {
    pub decl: DeclRef,
    pub value: Value,
    pub ty: Ty,
    pub symbol: Symbol,
    /// Its link section, when not the default one.
    pub section: Option<Symbol>,
}

/// Where the reach of a program starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub enum Roots {
    /// The root file's exports: the program as built.
    Exports,
    /// The exports and every function of the root file that depends on no compile-time
    /// binding and runs at run time: what coverage measures.
    Functions,
}

/// The program whose root is a file, reached from its roots.
pub struct Programs;

impl Query for Programs {
    const NAME: &'static str = "programs";
    type Key = (FileRef, Roots);
    type Value = Arc<Program>;

    fn compute(cx: &Cx<'_>, (root, roots): &(FileRef, Roots)) -> Self::Value {
        let mut reach =
            Reach { cx, program: Program::default(), seen: Map::default(), types: Set::default() };
        if let Some(parsed) = cx.get::<Parsed>(root)
            && let Some(ns) = parsed.hir.root
        {
            let hir = &parsed.hir;
            for decl in hir.member_decls(ns) {
                let decl = DeclRef::new(root.clone(), hir, decl);
                if let Ok(value) = cx.get::<DeclValue>(&decl) {
                    let ty = cx.get::<DeclType>(&decl).map(|known| known.ty);
                    reach.export(&decl, &value, ty.ok());
                }
            }
            if *roots == Roots::Functions {
                // A closure is reached from the body that makes it, with its captures.
                let open = hir.open_functions();
                for (id, e) in hir.exprs.iter() {
                    if !matches!(e.kind, ExprKind::Fn { .. })
                        || open.contains(&id)
                        || hir.closure_of(id).is_some()
                    {
                        continue;
                    }
                    let f = Instantiation::plain(ExprRef::new(root.clone(), hir, id));
                    if !f.compile_time(cx) {
                        reach.function(f);
                    }
                }
            }
        }
        // The functions reached in turn, each wave's bodies made over threads: the ones the
        // wave reaches come after it, in the order a function at a time would find them.
        let mut next = 0;
        while next < reach.program.functions.len() {
            let wave = reach.program.functions[next..].to_vec();
            next = reach.program.functions.len();
            for (i, body) in wave.iter().zip(cx.get_all::<Concrete>(&wave)) {
                reach.body(i, body);
            }
        }
        Arc::new(reach.program)
    }

    fn cycle(_: &Cx<'_>, _: &(FileRef, Roots), _: &Cycle) -> Self::Value {
        Arc::default()
    }
}

struct Reach<'a, 'db> {
    cx: &'a Cx<'db>,
    program: Program,
    /// The position of each function reached.
    seen: Map<Instantiation, usize>,
    types: Set<Ty>,
}

impl Reach<'_, '_> {
    /// Finds the exports among `value`, the value of `decl` of type `ty`: a function, the value
    /// itself as data, or, for a struct type, among its members.
    fn export(&mut self, decl: &DeclRef, value: &Value, ty: Option<Ty>) {
        match value {
            Value::Fn(f) => {
                if let Some(symbol) = f.export(self.cx) {
                    let at = self.function(Instantiation::new(f.clone(), Vec::new()));
                    self.program.exports.push((at, symbol));
                }
            }
            Value::Type(ty) if matches!(&**ty, TyKind::Nominal(_)) && self.types.insert(*ty) => {
                let Some(members) = self.cx.get::<Members>(ty) else { return };
                for member in &members.decls {
                    let value = self.cx.get::<MemberValue>(&(*ty, member.name));
                    let own = self.cx.get::<MemberType>(&(*ty, member.name)).map(|known| known.ty);
                    match (value, own) {
                        (Ok(value), own) => self.export(&member.decl, &value, own.ok()),
                        (Err(why), _) => tracing::debug!(member = %member.name, ?why, "member not evaluated"),
                    }
                }
            }
            _ => {
                let Some(symbol) = decl.export(self.cx) else { return };
                let Some(ty) = ty else {
                    tracing::debug!(%symbol, "exported datum of no known type");
                    return;
                };
                self.value(value);
                let section = decl.section(self.cx);
                let datum = ExportedDatum { decl: decl.clone(), value: value.clone(), ty, symbol, section };
                self.program.data.push(datum);
            }
        }
    }

    /// The position of `i` among the functions, reaching it if new.
    fn function(&mut self, i: Instantiation) -> usize {
        let functions = &mut self.program.functions;
        *self.seen.entry(i).or_insert_with_key(|i| {
            functions.push(i.clone());
            functions.len() - 1
        })
    }

    /// Reaches what `body`, the body of `i`, calls and takes.
    fn body(&mut self, i: &Instantiation, body: <Concrete as Query>::Value) {
        let body = match body {
            Ok(body) => body,
            Err(unready) => return self.program.unready.push((i.clone(), unready)),
        };
        // The body's environment, which only a closure made needs, made once for the first.
        let mut env = None;
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                // A called function is reached as the call instantiates it; what a statement
                // passes or reads is taken as a value.
                let read = match &stmt.value {
                    Rvalue::Call { callee: Operand::Const(Value::Fn(f)), args } => {
                        self.function(Instantiation::called(self.cx, f, args));
                        args.iter().collect()
                    }
                    Rvalue::Aggregate { kind: Aggregate::Closure { literal, staged }, parts } => {
                        if let Some(env) = env.get_or_insert_with(|| i.env(self.cx)) {
                            let f = env.clone().staging(staged).func(*literal);
                            self.function(Instantiation::new(f, Vec::new()));
                        }
                        parts.iter().collect()
                    }
                    value => value.operands(),
                };
                for operand in read {
                    self.operand(operand);
                }
            }
            for operand in block.term.kind.operands() {
                self.operand(operand);
            }
        }
    }

    fn operand(&mut self, operand: &Operand) {
        if let Operand::Const(value) = operand {
            self.value(value);
        }
    }

    /// The functions and link symbols a constant holds, taken as values.
    fn value(&mut self, value: &Value) {
        match value {
            Value::Fn(f) | Value::Closure { f, .. } => {
                self.function(Instantiation::new(f.clone(), Vec::new()));
            }
            Value::Linked(decl) if !self.program.imports.contains(decl) => {
                self.program.imports.push(decl.clone());
            }
            Value::Constructor { ty, name } if !self.program.constructors.contains(&(*ty, *name)) => {
                self.program.constructors.push((*ty, *name));
            }
            _ => {}
        }
        let parts: Vec<&Value> = match value {
            Value::Tuple(items) | Value::Array(items) | Value::Variant { payload: items, .. } => {
                items.iter().collect()
            }
            Value::Closure { captures, .. } => captures.iter().collect(),
            Value::Struct { fields, .. } => fields.iter().map(|(_, v)| v).collect(),
            _ => Vec::new(),
        };
        for part in parts {
            self.value(part);
        }
    }
}
