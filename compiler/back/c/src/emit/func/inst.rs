//! Instructions: a value's expression is kept for its one use or assigned to its local; memory is
//! reached by typed access through `u8 *` addresses, which `-fno-strict-aliasing` defines.

use vun_lir::func::Def;
use vun_lir::{
    AbiType, Access, CallKind, Callee, Conv, ImportKind, Inst, InstKind, Order, RmwOp, Signature, Value,
};
use vun_target::{Atomics, TailCalls};

use super::FnEmit;
use crate::ast::{Binary, Builtin, Expr, Ident, Own, Stmt, Ty, Unary};

use crate::emit::signature::SignedC;
use vun_backend::{Lack, Missing};

impl FnEmit<'_, '_> {
    pub(super) fn inst(&mut self, inst: Inst) -> Result<(), Lack> {
        let func = self.func;
        let data = &func.insts[inst];
        let at = data.origin;
        self.at(at);
        let value = match &data.kind {
            &InstKind::Const { ty, bits } => self.e.constant(ty, bits, at)?,
            &InstKind::Binary { op, a, b } => self.binary(op, a, b)?,
            &InstKind::Compare { cond, a, b } => self.compare(cond, a, b),
            &InstKind::Cast { op, value, to } => self.cast(op, value, to)?,
            &InstKind::Select { cond, then, otherwise } => {
                let c = self.operand(cond).truth();
                Expr::Select(Box::new(c), Box::new(self.operand(then)), Box::new(self.operand(otherwise)))
            }
            &InstKind::Load { ty, addr, access } => {
                let ty = self.e.scalar(ty, at)?;
                match access {
                    Access::Plain => self.operand(addr).at(ty),
                    Access::Volatile => self.operand(addr).at(Ty::Volatile(Box::new(ty))),
                    Access::Atomic(order) => {
                        let place = self.operand(addr).cast(ty.pointer());
                        self.atomic(Builtin::Load, vec![place], &[order], at)?
                    }
                }
            }
            &InstKind::Store { value, addr, access } => {
                let ty = self.e.scalar(func.ty(value), at)?;
                let stored = self.operand(value);
                let stmt = match access {
                    Access::Plain => Stmt::Assign(self.operand(addr).at(ty), stored),
                    Access::Volatile => {
                        Stmt::Assign(self.operand(addr).at(Ty::Volatile(Box::new(ty))), stored)
                    }
                    Access::Atomic(order) => {
                        let place = self.operand(addr).cast(ty.pointer());
                        Stmt::Expr(self.atomic(Builtin::Store, vec![place, stored], &[order], at)?)
                    }
                };
                self.body.push(stmt);
                return Ok(());
            }
            &InstKind::Rmw { op, addr, value, order } => {
                let ty = self.e.scalar(func.ty(value), at)?;
                let builtin = match op {
                    RmwOp::Swap => Builtin::Exchange,
                    RmwOp::Add => Builtin::FetchAdd,
                    RmwOp::Sub => Builtin::FetchSub,
                    RmwOp::And => Builtin::FetchAnd,
                    RmwOp::Or => Builtin::FetchOr,
                    RmwOp::Xor => Builtin::FetchXor,
                };
                let place = self.operand(addr).cast(ty.pointer());
                let value = self.operand(value);
                self.atomic(builtin, vec![place, value], &[order], at)?
            }
            // The builtin writes what the place held into the expected value's place when they
            // differ: the result's local, which holds the value held before either way.
            &InstKind::CompareExchange { addr, expected, new, success, failure } => {
                let result = data.result.expect("a compare-exchange has a value");
                let ty = self.e.scalar(func.ty(new), at)?;
                let place = self.operand(addr).cast(ty.pointer());
                let expected = self.operand(expected);
                self.assign(result, expected);
                let held = Expr::unary(Unary::Address, Expr::Name(Ident::value(result)));
                let (new, strong) = (self.operand(new), Expr::Small(0));
                let call = self.atomic(
                    Builtin::CompareExchange,
                    vec![place, held, new, strong],
                    &[success, failure],
                    at,
                )?;
                self.body.push(Stmt::Expr(call));
                return Ok(());
            }
            // C defines pointer arithmetic only inside an object or one past its end, which is
            // exactly where LIR keeps an offset (spec 13, section 3; decision 245).
            &InstKind::Offset { base, offset } => {
                let base = self.operand(base);
                let offset = self.offset(offset);
                Expr::binary(Binary::Add, base, offset)
            }
            &InstKind::SlotAddr(slot) => Expr::Name(Ident::slot(slot)).cast(Ty::address()),
            // An object `__thread` declares is the calling thread's, where C names it.
            &InstKind::Addr(address) => self.e.address(address),
            &InstKind::ThreadAddr(data) => self.e.address(vun_lir::Address::Data(data)),
            &InstKind::Copy { to, from, bytes } => {
                self.e.copies.insert(bytes);
                let ty = Ty::Struct(Ident::Own(Own::Copy(bytes)));
                let (to, from) = (self.operand(to).at(ty.clone()), self.operand(from).at(ty));
                self.body.push(Stmt::Assign(to, from));
                return Ok(());
            }
            InstKind::Call { callee, args, kind } => return self.call(callee, args, *kind, data.result),
            InstKind::Asm(asm, check) => return self.asm(inst, asm, check.as_ref(), at),
        };
        if let Some(result) = data.result {
            self.define(result, value);
        }
        Ok(())
    }

    /// A call of the atomic `builtin` with `args` then `orders`, when the target's C compiler
    /// has the builtins.
    fn atomic(
        &mut self,
        builtin: Builtin,
        args: Vec<Expr>,
        orders: &[Order],
        at: vun_origin::OriginId,
    ) -> Result<Expr, Lack> {
        if self.e.target.c.compiler.atomics != Atomics::Gnu {
            return Err(Missing::Atomic.at(at));
        }
        let orders = orders.iter().map(|&o| {
            self.e.builtins.insert(Builtin::Order(o));
            Expr::Name(Ident::Builtin(Builtin::Order(o)))
        });
        let args = args.into_iter().chain(orders.collect::<Vec<_>>()).collect();
        self.e.builtins.insert(builtin);
        Ok(Expr::Call(Box::new(Expr::Name(Ident::Builtin(builtin))), args))
    }

    /// An offset added to an address, read as signed: a constant one as a plain literal.
    fn offset(&mut self, offset: Value) -> Expr {
        let bits = self.e.unit.pointer_bits;
        if let Def::Inst(def) = self.func.values[offset].def
            && let InstKind::Const { bits: raw, .. } = self.func.insts[def].kind
        {
            let shift = 128 - bits;
            #[expect(clippy::cast_possible_wrap, reason = "the offset's bits read as signed")]
            let signed = ((raw << shift) as i128) >> shift;
            if let Ok(small) = i64::try_from(signed) {
                self.exprs[offset] = None;
                return Expr::Small(small);
            }
        }
        self.operand(offset).cast(Ty::int(bits))
    }

    /// A call, its arguments and result converted as the callee's convention passes them.
    fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        kind: CallKind,
        result: Option<Value>,
    ) -> Result<(), Lack> {
        let unit = self.e.unit;
        let at = self.func.origin;
        let (name, signature) = match callee {
            &Callee::Func(f) => (Expr::Name(self.e.func(f)), unit.functions[f].signature.clone()),
            &Callee::Import(i) => {
                let import = &unit.imports[i];
                let ImportKind::Func(signature) = &import.kind else {
                    unreachable!("the verifier checks callees")
                };
                (Expr::Name(Ident::Symbol(import.symbol)), signature.clone())
            }
            Callee::Indirect(f, signature) => {
                let ty = self.e.signature(signature, at)?.pointer();
                (self.operand(*f).cast(self.e.word()).cast(ty), (**signature).clone())
            }
        };
        let c = self.e.signature(&signature, at)?;
        let mut passed = Vec::new();
        let mut args = args.iter().copied();
        let mut types = c.params.iter();
        for abi in &signature.params {
            // Under Vun's convention a wide integer's limbs are arguments of their own.
            if let &AbiType::Wide { limbs, .. } = abi
                && signature.conv != Conv::C
            {
                for _ in 0..limbs {
                    passed.push(self.operand(args.next().expect("a call passes the signature's values")));
                    types.next();
                }
                continue;
            }
            let ty = types.next().expect("a C parameter for each value");
            let arg = self.operand(args.next().expect("a call passes the signature's values"));
            passed.push(match abi {
                AbiType::Record(_) => arg.at(ty.clone()),
                &AbiType::Wide { limb: half, .. } => {
                    let high = self.operand(args.next().expect("a wide integer is two values"));
                    self.e.join(ty.clone(), [arg, high], half, at)?
                }
                _ if abi.signed_c(signature.conv).is_some() => arg.cast(ty.clone()),
                _ => arg,
            });
        }
        let out = args.next();
        let call = |passed| Expr::Call(Box::new(name), passed);
        if kind == CallKind::Tail {
            return self.tail_call(&signature, passed, call);
        }
        match &signature.result {
            Some(AbiType::Memory { .. }) => {
                passed.push(self.operand(out.expect("a result in memory has an address")));
                self.body.push(Stmt::Expr(call(passed)));
            }
            Some(AbiType::Record(_) | AbiType::Wide { .. }) => {
                let place =
                    self.operand(out.expect("a result through memory has an address")).at(c.result.clone());
                self.body.push(Stmt::Assign(place, call(passed)));
            }
            Some(given @ (AbiType::Scalar(_) | AbiType::Int(..) | AbiType::Address(_))) => {
                let value = match given.signed_c(signature.conv) {
                    Some(s) => call(passed).cast(self.e.scalar(s, at)?),
                    None => call(passed),
                };
                match result {
                    Some(v) if self.live.range(v).is_some_and(|r| r.uses > 0) => self.assign(v, value),
                    _ => self.body.push(Stmt::Expr(value)),
                }
            }
            None => self.body.push(Stmt::Expr(call(passed))),
        }
        Ok(())
    }

    /// A call the C compiler must turn into a jump: the `return` after it makes it.
    fn tail_call(
        &mut self,
        signature: &Signature,
        passed: Vec<Expr>,
        call: impl FnOnce(Vec<Expr>) -> Expr,
    ) -> Result<(), Lack> {
        let at = self.func.origin;
        if self.e.target.c.compiler.tail_calls != TailCalls::Musttail {
            return Err(Missing::TailCall.at(at));
        }
        let call = match &signature.result {
            Some(AbiType::Memory { .. } | AbiType::Record(_) | AbiType::Wide { .. }) => {
                return Err(Missing::TailCall.at(at));
            }
            Some(given) if let Some(s) = given.signed_c(signature.conv) => {
                call(passed).cast(self.e.scalar(s, at)?)
            }
            _ => call(passed),
        };
        self.tail = Some(call);
        Ok(())
    }
}
