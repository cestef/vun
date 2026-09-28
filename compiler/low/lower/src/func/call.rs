//! Calls: of a function reached by the program, of a closure, of a function value, of a link
//! symbol, or of a primitive. Each run-time
//! argument is written into a temporary of its parameter's type, passed as its scalar or by its
//! address; a result in memory is written through an address, converted into the place if its
//! type differs.

use vun_lir::{Callee, Conv, InstKind, Signature, Value};
use vun_mir::{Concrete, Lowered as _, Operand, Projection};
use vun_sema::Primitive;
use vun_sema::decl::DeclType;
use vun_sema::eval::Instantiation;
use vun_sema::int::Integer;
use vun_sema::layout::Parts;
use vun_sema::primitives::abi::Abi;
use vun_sema::primitives::check::Check;
use vun_sema::primitives::panic::Panic;
use vun_sema::trap::Failure;
use vun_sema::ty::{Marker, Ty, TyKind, Value as Constant};

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
                let body =
                    cx.get::<Concrete>(&i).map_err(|u| Unlowered { at: u.at(), why: Why::Unready(u) })?;
                // Calling a coroutine function runs nothing: it builds the frame.
                if body.suspends() {
                    return self.construct(&i, &body, args, dest, ty);
                }
                let id = self.l.function(&i, self.at)?;
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
                let abi = decl.linked(cx).and_then(|l| l.abi);
                let args: Vec<&Operand> = args.iter().collect();
                self.invoke(callee, &signature.params, signature.result, abi, &args, dest, ty)
            }
            // `lang.panic.fail(reason)` is a trap site saying its reason, known when compiling.
            Operand::Const(Constant::Primitive(Primitive::Panic(Panic::fail))) => {
                let [Operand::Const(reason)] = args else { return Err(unsupported()) };
                self.site(Some(Failure::of(reason).ok_or_else(unsupported)?.msg()))
            }
            // `lang.check.site(at)` is a new site at `at`, known when compiling: its id.
            Operand::Const(Constant::Primitive(Primitive::Check(Check::site))) => {
                let [Operand::Const(Constant::Origin(at))] = args else { return Err(unsupported()) };
                let site = self.new_site(*at, None)?;
                let Some(dest) = dest else { return Ok(()) };
                self.write_const(&Constant::Int(Integer::from(site as usize)), dest, ty)
            }
            Operand::Const(Constant::Primitive(p)) => {
                self.primitive(*p, args, dest, ty).map_err(|u| u.naming(*p))
            }
            // A closure runs its function, which takes the captures before the arguments.
            Operand::Const(Constant::Closure { f, captures }) => {
                let captured = captures.iter().cloned().map(Operand::Const);
                let args: Vec<Operand> = captured.chain(args.iter().cloned()).collect();
                self.call(&Operand::Const(Constant::Fn(f.clone())), &args, dest, ty)
            }
            Operand::Place(place) => {
                let callee_ty = self.operand_ty(callee).ok_or_else(unsupported)?;
                if let TyKind::Closure(closure) = &*callee_ty {
                    let captured = (0..closure.captures.len())
                        .map(|i| Operand::Place(place.clone().project(Projection::Position(i))));
                    let args: Vec<Operand> = captured.chain(args.iter().cloned()).collect();
                    return self.call(&Operand::Const(Constant::Fn(closure.f.clone())), &args, dest, ty);
                }
                // A function value is its address, called with its type's signature.
                let signature = callee_ty.signature().ok_or_else(unsupported)?;
                let abi = Marker::abi(&signature.markers);
                let pointer = self.scalar_operand(callee)?;
                let lowered = self.l.signature(&signature.params, signature.result, abi, self.at)?;
                let callee = Callee::Indirect(pointer, Box::new(lowered));
                let args: Vec<&Operand> = args.iter().collect();
                self.invoke(callee, &signature.params, signature.result, abi, &args, dest, ty)
            }
            Operand::Const(_) => Err(unsupported()),
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
            // A pack passes the tuple of its arguments; under the C convention, each of its items
            // as a variadic argument.
            let packed = param.packed();
            let param = packed.unwrap_or(param);
            let Some(temp) = self.temp(param)? else { continue };
            self.write(arg, Some(temp), param)?;
            if packed.is_some() && signature.conv == Conv::C {
                let shape = self.l.types.shape(param, self.at)?;
                let Parts::Sequence(offsets) = &shape.parts else {
                    return Err(Unlowered::unsupported(self.at, Form::Callee));
                };
                let items = self.l.types.parts(param).ok_or(Unlowered::unsupported(self.at, Form::Callee))?;
                for (item, &offset) in items.into_iter().zip(offsets) {
                    let at = self.at_offset(temp, offset);
                    self.pass(at, item, &mut values)?;
                }
            } else {
                self.pass(temp, param, &mut values)?;
            }
        }
        // The result lands in `dest` when it has the place's type, else in a temporary first. A C
        // callee may write the result it returns through memory before reading what it is passed,
        // which may reach `dest`, so its result lands in a temporary too.
        let aliased = signature.through() && signature.conv == Conv::C;
        let landing = if result == ty && dest.is_some() && !aliased { dest } else { self.temp(result)? };
        if signature.through() {
            values.extend(landing);
        }
        let value = self.b.push(InstKind::Call { callee, args: values }, signature.returns(), self.at);
        if let (Some(value), Some(landing)) = (value, landing) {
            self.b.store(value, landing, vun_lir::Access::Plain, self.at);
        }
        if let (Some(landing), Some(dest)) = (landing, dest)
            && landing != dest
        {
            if result == ty {
                self.copy_value(landing, dest, ty)?;
            } else {
                self.convert(landing, result, dest, ty)?;
            }
        }
        Ok(())
    }

    /// Passes the value of type `ty` at `at`: its scalar, or its address.
    fn pass(&mut self, at: Value, ty: Ty, values: &mut Vec<Value>) -> Result<(), Unlowered> {
        match self.l.types.repr(ty, self.at)? {
            Repr::Scalar(s) => values.push(self.b.load(s, at, vun_lir::Access::Plain, self.at)),
            Repr::Memory(_) => values.push(at),
            Repr::Zero => {}
        }
        Ok(())
    }
}
