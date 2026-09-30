//! How a program's functions are found: from its roots, through what each body calls and takes.

use vun_hash::{Map, Set};
use vun_query::{Cx, Query};
use vun_sema::link::{Exported, Placed};
use vun_sema::member::{MemberType, MemberValue, Members};
use vun_sema_ty::call::Instantiation;
use vun_sema_ty::item::DeclRef;
use vun_sema_ty::trap::Unevaluated;
use vun_sema_ty::{Ty, TyKind, Value};

use super::{ExportedDatum, Program, Reached, Statement, UnevaluatedExport};
use crate::{Aggregate, Concrete, Lowered, Operand, Rvalue};

pub(super) struct Reach<'a, 'db> {
    cx: &'a Cx<'db>,
    pub(super) program: Program,
    /// The position of each function reached.
    seen: Map<Instantiation, usize>,
    types: Set<Ty>,
    /// The statement being read, which reaches what it calls and takes; `None` for a root.
    by: Option<Statement>,
}

impl<'a, 'db> Reach<'a, 'db> {
    pub(super) fn new(cx: &'a Cx<'db>) -> Self {
        Self { cx, program: Program::default(), seen: Map::default(), types: Set::default(), by: None }
    }

    /// Finds the exports among `value`, the value of `decl` of type `ty`: a function, the value
    /// itself as data, or, for a struct type, among its members. An exported declaration with no
    /// value is recorded, for lowering to report.
    pub(super) fn export(
        &mut self,
        decl: &DeclRef,
        value: Result<Value, Unevaluated>,
        ty: Result<Ty, Unevaluated>,
    ) {
        let value = match value {
            Ok(value) => value,
            Err(why) => return self.unevaluated(decl, why),
        };
        match &value {
            // Exported by its declaration, or by the one of the function it names.
            Value::Fn(f) => {
                if let Some(symbol) = decl.export(self.cx).or_else(|| f.export(self.cx)) {
                    let at = self.function(Instantiation::new(f.clone(), Vec::new()));
                    self.program.exports.push((at, symbol));
                }
            }
            Value::Type(ty) if matches!(&**ty, TyKind::Nominal(_)) && self.types.insert(*ty) => {
                let Some(members) = self.cx.get::<Members>(ty) else { return };
                for member in &members.decls {
                    let value = self.cx.get::<MemberValue>(&(*ty, member.name));
                    let own = self.cx.get::<MemberType>(&(*ty, member.name)).map(|known| known.ty);
                    self.export(&member.decl, value, own);
                }
            }
            _ => {
                let Some(symbol) = decl.export(self.cx) else { return };
                let ty = match ty {
                    Ok(ty) => ty,
                    Err(why) => return self.unevaluated(decl, why),
                };
                self.value(&value);
                let section = decl.section(self.cx);
                let datum = ExportedDatum { decl: decl.clone(), value, ty, symbol, section };
                self.program.data.push(datum);
            }
        }
    }

    /// Records `decl`, with no value or type at compile time, if it is exported.
    fn unevaluated(&mut self, decl: &DeclRef, why: Unevaluated) {
        if let Some(symbol) = decl.export(self.cx) {
            self.program.unevaluated.push(UnevaluatedExport { decl: decl.clone(), symbol, why });
        }
    }

    /// The position of `i` among the functions, reaching it by the statement read if new.
    pub(super) fn function(&mut self, i: Instantiation) -> usize {
        let (functions, by) = (&mut self.program.functions, self.by);
        *self.seen.entry(i).or_insert_with_key(|i| {
            functions.push(Reached { i: i.clone(), by });
            functions.len() - 1
        })
    }

    /// Reaches what `body`, the body of `i`, calls and takes.
    pub(super) fn body(&mut self, i: &Instantiation, body: <Concrete as Query>::Value) {
        let body = match body {
            Ok(body) => body,
            Err(unready) => return self.program.unready.push((i.clone(), unready)),
        };
        // The body's environment, which only a closure made needs, made once for the first.
        let mut env = None;
        let function = self.seen[i];
        for (_, block) in body.blocks.iter() {
            for stmt in &block.stmts {
                self.by = Some(Statement { function, at: stmt.origin });
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
            self.by = Some(Statement { function, at: block.term.origin });
            for operand in block.term.kind.operands() {
                self.operand(operand);
            }
        }
        self.by = None;
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
