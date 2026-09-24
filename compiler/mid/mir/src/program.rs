//! A program (spec 08, section 1): the functions its root file's exports reach. The exports are
//! the `#[link.export]` functions among the root file's bindings and the members of the struct
//! types they evaluate to; from each, every function a body calls or takes as a value is reached,
//! with the `$` arguments of its call.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use vun_hir::db::{FileRef, Parsed};
use vun_intern::Symbol;
use vun_query::{Cx, Cycle, Query};
use vun_sema::decl::DeclValue;
use vun_sema::eval::Instantiation;
use vun_sema::item::DeclRef;
use vun_sema::member::{MemberValue, Members};
use vun_sema::ty::{Ty, TyKind, Value};

use crate::{Aggregate, Concrete, Lowered, Operand, Rvalue, Unready};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Program {
    /// Every function reached, in the order found: the exports first.
    pub functions: Vec<Instantiation>,
    /// The exported functions, by their position in `functions`, and the symbol of each.
    pub exports: Vec<(usize, Symbol)>,
    /// The declarations the linker provides that a body reaches.
    pub imports: Vec<DeclRef>,
    /// The functions reached whose bodies are not ready, and why.
    pub unready: Vec<(Instantiation, Unready)>,
}

/// The program whose root is a file.
pub struct Programs;

impl Query for Programs {
    const NAME: &'static str = "programs";
    type Key = FileRef;
    type Value = Arc<Program>;

    fn compute(cx: &Cx<'_>, root: &FileRef) -> Self::Value {
        let mut reach =
            Reach { cx, program: Program::default(), seen: HashMap::new(), types: HashSet::new() };
        if let Some(parsed) = cx.get::<Parsed>(root)
            && let Some(ns) = parsed.hir.root
        {
            for decl in parsed.hir.member_decls(ns) {
                if let Ok(value) = cx.get::<DeclValue>(&DeclRef::new(root.clone(), &parsed.hir, decl)) {
                    reach.export(&value);
                }
            }
        }
        let mut next = 0;
        while let Some(i) = reach.program.functions.get(next).cloned() {
            reach.body(&i);
            next += 1;
        }
        Arc::new(reach.program)
    }

    fn cycle(_: &Cx<'_>, _: &FileRef, _: &Cycle) -> Self::Value {
        Arc::default()
    }
}

struct Reach<'a, 'db> {
    cx: &'a Cx<'db>,
    program: Program,
    /// The position of each function reached.
    seen: HashMap<Instantiation, usize>,
    types: HashSet<Ty>,
}

impl Reach<'_, '_> {
    /// Finds the exports among `value` and, for a struct type, among its members.
    fn export(&mut self, value: &Value) {
        match value {
            Value::Fn(f) => {
                if let Some(symbol) = f.export(self.cx) {
                    let at = self.function(Instantiation::new(f.clone(), Vec::new()));
                    self.program.exports.push((at, symbol));
                }
            }
            Value::Type(ty) if matches!(&**ty, TyKind::Nominal(_)) && self.types.insert(*ty) => {
                let Some(members) = self.cx.get::<Members>(ty) else { return };
                for decl in &members.decls {
                    if let Ok(value) = self.cx.get::<MemberValue>(&(*ty, decl.name)) {
                        self.export(&value);
                    }
                }
            }
            _ => {}
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

    /// Reaches what the body of `i` calls and takes.
    fn body(&mut self, i: &Instantiation) {
        let body = match self.cx.get::<Concrete>(i) {
            Ok(body) => body,
            Err(unready) => return self.program.unready.push((i.clone(), unready)),
        };
        let env = i.env(self.cx);
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
                        if let Some(env) = &env {
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
