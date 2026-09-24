//! Calls: of a function reached by the program, of a link symbol, or of a primitive. Each run-time
//! argument is written into a temporary of its parameter's type, passed as its scalar or by its
//! address; a result in memory is written through an address, converted into the place if its
//! type differs.

use vun_lir::{Callee, InstKind, Signature, Value};
use vun_mir::{Concrete, Lowered as _, Operand};
use vun_sema::decl::DeclType;
use vun_sema::eval::Instantiation;
use vun_sema::primitives::abi::Abi;
use vun_sema::ty::{Ty, Value as Constant};

use super::FnLower;
use crate::repr::Repr;
use crate::unlowered::{Form, Unlowered, Why};

impl FnLower<'_, '_, '_> {
    pub(crate) fn call(
        &mut self,
        callee: &Operand,
        args: &[Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let cx = self.l.types.cx;
        let at = self.at;
        let unsupported = move || Unlowered::unsupported(at, Form::Callee);
        match callee {
            Operand::Const(Constant::Fn(f)) => {
                let i = Instantiation::called(cx, f, args);
                let id = self.l.function(&i, self.at)?;
                let body =
                    cx.get::<Concrete>(&i).map_err(|u| Unlowered { at: u.at(), why: Why::Unready(u) })?;
                let comptime = f.comptime(cx).unwrap_or_default();
                let runtime = |i: &usize| comptime.get(*i) != Some(&true);
                let params = body
                    .params
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| runtime(i))
                    .map(|(_, &p)| body.locals[p].ty);
                let params = params.collect::<Option<Vec<Ty>>>().ok_or_else(unsupported)?;
                let args: Vec<&Operand> =
                    args.iter().enumerate().filter(|(i, _)| runtime(i)).map(|(_, a)| a).collect();
                let result = body.locals[body.ret].ty.ok_or_else(unsupported)?;
                self.invoke(Callee::Func(id), &params, result, f.abi(cx), &args, dest, ty)
            }
            // A `lang` primitive the linker provides, such as `lang.panic.trap`, is its symbol.
            Operand::Const(Constant::Primitive(p))
                if let Ok(decl) = self.l.env.primitive_decl(*p)
                    && decl.linked(cx).is_some() =>
            {
                self.call(&Operand::Const(Constant::Linked(decl)), args, dest, ty)
            }
            Operand::Const(Constant::Linked(decl)) => {
                let callee = self.l.linked(decl, self.at)?.callee().ok_or_else(unsupported)?;
                let signature =
                    cx.get::<DeclType>(decl).ok().and_then(|k| k.ty.signature()).ok_or_else(unsupported)?;
                let abi = decl.linked(cx).and_then(|(_, abi)| abi);
                let args: Vec<&Operand> = args.iter().collect();
                self.invoke(callee, &signature.params, signature.result, abi, &args, dest, ty)
            }
            Operand::Const(Constant::Primitive(p)) => self.primitive(*p, args, dest, ty),
            _ => Err(unsupported()),
        }
    }

    /// Calls `callee`, whose parameters and result have the types `params` and `result` under
    /// `abi`, with `args`, writing the result into `dest` as a value of type `ty`.
    #[expect(clippy::too_many_arguments, reason = "a call is its callee, its signature and its operands")]
    fn invoke(
        &mut self,
        callee: Callee,
        params: &[Ty],
        result: Ty,
        abi: Option<Abi>,
        args: &[&Operand],
        dest: Option<Value>,
        ty: Ty,
    ) -> Result<(), Unlowered> {
        let signature: Signature = self.l.signature(params, result, abi, self.at)?;
        let mut values = Vec::new();
        for (&param, &arg) in params.iter().zip(args) {
            let Some(temp) = self.temp(param)? else { continue };
            self.write(arg, Some(temp), param)?;
            match self.l.types.repr(param, self.at)? {
                Repr::Scalar(s) => values.push(self.b.load(s, temp, vun_lir::Access::Plain, self.at)),
                Repr::Memory(_) => values.push(temp),
                Repr::Zero => {}
            }
        }
        // The result lands in `dest` when it has the place's type, else in a temporary first.
        let landing = if result == ty && dest.is_some() { dest } else { self.temp(result)? };
        if signature.through() {
            values.extend(landing);
        }
        let value = self.b.push(InstKind::Call { callee, args: values }, signature.returns(), self.at);
        if let (Some(value), Some(landing)) = (value, landing) {
            self.b.store(value, landing, vun_lir::Access::Plain, self.at);
        }
        if result != ty
            && let (Some(landing), Some(dest)) = (landing, dest)
        {
            self.convert(landing, result, dest, ty)?;
        }
        Ok(())
    }
}
