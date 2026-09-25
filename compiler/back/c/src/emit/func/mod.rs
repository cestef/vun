//! A function's body: locals declared first, then each reached block in `Order`, a block's label
//! written only where a `goto` names it.

mod arith;
mod inline;
mod inst;
mod term;

use std::collections::BTreeSet;

use vun_backend::Line;
use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Liveness, Loops, Order};
use vun_lir::{AbiType, Block, Conv, Ext, FuncId, Function, Scalar, Value};
use vun_origin::OriginId;

use super::Emitter;
use super::signature::CSignature;
use crate::ast::{Decl, Expr, Ident, Item, Own, Proto, Stmt, Storage, Ty};
use inline::Inlined;
use vun_backend::Lack;

pub(super) struct FnEmit<'e, 'u> {
    e: &'e mut Emitter<'u>,
    func: &'u Function,
    order: Order,
    live: Liveness,
    inlined: Inlined,
    /// The expressions of inlined values, taken by their one use.
    exprs: IndexVec<Value, Option<Expr>>,
    locals: BTreeSet<Value>,
    temps: BTreeSet<Scalar>,
    body: Vec<Stmt>,
    line: Option<Line>,
    /// The `struct` a record result is built in, under the C convention.
    result: Option<Ty>,
}

impl Emitter<'_> {
    pub(super) fn define(&mut self, id: FuncId, storage: Storage) -> Result<Item, Lack> {
        let unit = self.unit;
        let func = &unit.functions[id];
        let _span = vun_trace::tracing::debug_span!("c", f = %func.linkage).entered();
        let c = self.signature(&func.signature, func.origin)?;
        let loops = Loops::of(func, &Cfg::of(func));
        let order = Order::of(func, &loops);
        let live = Liveness::of(func, &loops, &order);
        let inlined = Inlined::of(func, &order, &live);
        let name = self.func(id);
        let mut f = FnEmit {
            e: self,
            func,
            order,
            live,
            inlined,
            exprs: IndexVec::filled(None, func.values.len()),
            locals: BTreeSet::new(),
            temps: BTreeSet::new(),
            body: Vec::new(),
            line: None,
            result: None,
        };
        let params = f.entry(&c)?;
        for block in f.order.blocks().to_vec() {
            f.block(block)?;
        }
        let body = f.finish()?;
        Ok(Item::Define(Proto { storage, result: c.result, name, params }, body))
    }
}

impl FnEmit<'_, '_> {
    /// The C parameters: each is the entry block's value where C passes it as the body reads it,
    /// or else is converted into that value first.
    fn entry(&mut self, c: &CSignature) -> Result<Vec<Decl>, Lack> {
        let func = self.func;
        let Some(entry) = func.entry() else { return Ok(Vec::new()) };
        let values = &func.blocks[entry].params;
        let signature = &func.signature;
        let mut params = Vec::new();
        for (i, (abi, ty)) in signature.params.iter().zip(&c.params).enumerate() {
            let (v, param) = (values[i], Ident::param(i));
            let converted = match abi {
                AbiType::Record(_) => Expr::Name(param).address_of(),
                &AbiType::Int(s, Ext::Sign) if signature.conv == Conv::C => {
                    Expr::Name(param).cast(self.e.scalar(s, func.origin)?)
                }
                AbiType::Scalar(_) | AbiType::Int(..) | AbiType::Address(_) | AbiType::Memory { .. } => {
                    params.push(Decl::new(ty.clone(), Ident::value(v)));
                    continue;
                }
            };
            params.push(Decl::new(ty.clone(), param));
            self.assign(v, converted);
        }
        let out = values.last().copied();
        match (&signature.result, out) {
            (Some(AbiType::Memory { .. }), Some(out)) => {
                params.push(Decl::new(Ty::address(), Ident::value(out)));
            }
            (Some(AbiType::Record(_)), Some(out)) => {
                self.result = Some(c.result.clone());
                self.assign(out, Expr::Name(Ident::Own(Own::Result)).address_of());
            }
            _ => {}
        }
        Ok(params)
    }

    fn block(&mut self, block: Block) -> Result<(), Lack> {
        let data = &self.func.blocks[block];
        if Some(block) != self.func.entry() {
            self.locals.extend(data.params.iter().copied());
            self.body.push(Stmt::Label(Ident::block(block)));
        }
        for &inst in &data.insts {
            self.inst(inst)?;
        }
        self.term(block)
    }

    /// Places the statements that follow at `origin`'s line.
    fn at(&mut self, origin: OriginId) {
        if let Some(line) = self.e.lines.line(origin)
            && self.line.as_ref() != Some(&line)
        {
            self.body.push(Stmt::Line(line.clone()));
            self.line = Some(line);
        }
    }

    /// Writes `expr` into the local of `v`.
    fn assign(&mut self, v: Value, expr: Expr) {
        self.locals.insert(v);
        self.body.push(Stmt::Assign(Expr::Name(Ident::value(v)), expr));
    }

    /// The expression reading `v`: its own, if inlined, or its local.
    fn operand(&mut self, v: Value) -> Expr {
        self.exprs[v].take().unwrap_or(Expr::Name(Ident::value(v)))
    }

    /// Defines `v` as `expr`: kept for its one use, or assigned to its local, or dropped when
    /// nothing reads it.
    fn define(&mut self, v: Value, expr: Expr) {
        if self.inlined.contains(v) {
            self.exprs[v] = Some(expr);
        } else if self.live.range(v).is_some_and(|r| r.uses > 0) {
            self.assign(v, expr);
        }
    }

    fn finish(self) -> Result<Vec<Stmt>, Lack> {
        let (func, e) = (self.func, &*self.e);
        let mut out = Vec::new();
        for &v in &self.locals {
            out.push(Stmt::Local(Decl::new(e.scalar(func.ty(v), func.origin)?, Ident::value(v))));
        }
        for &s in &self.temps {
            out.push(Stmt::Local(Decl::new(e.scalar(s, func.origin)?, Ident::Own(Own::Temp(s)))));
        }
        for (slot, data) in func.slots.iter() {
            let (ty, bytes) = e.aligned(data.align, data.origin)?;
            let len = data.size.div_ceil(bytes).max(1);
            out.push(Stmt::Local(Decl { ty, name: Ident::slot(slot), len: Some(len) }));
        }
        if let Some(ty) = self.result {
            out.push(Stmt::Local(Decl::new(ty, Ident::Own(Own::Result))));
        }
        let mut targets = BTreeSet::new();
        Stmt::targets(&self.body, &mut targets);
        out.extend(self.body.into_iter().filter(|s| !matches!(s, Stmt::Label(l) if !targets.contains(l))));
        Ok(out)
    }
}
