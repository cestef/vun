//! Calls, and the addresses of functions, data and storage per thread.

use cranelift_codegen::ir::immediates::Imm64;
use cranelift_codegen::ir::{
    ExtFuncData, ExternalName, FuncRef, GlobalValue, GlobalValueData, InstBuilder, Value as Val,
};
use cranelift_codegen::isa::CallConv;
use vun_backend::{Lack, Missing};
use vun_lir::{Address, Callee, DataId, FuncId, ImportKind, Value};
use vun_origin::OriginId;

use super::Lower;
use crate::abi::Plan;
use crate::names::{Body, Named};
use crate::plan::Returned;

impl Lower<'_, '_> {
    /// Calls `callee`, giving its result if `defines` says the instruction has one.
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        defines: bool,
        at: OriginId,
    ) -> Result<Option<Val>, Lack> {
        let cx = self.cx;
        let owned;
        let plan = match callee {
            Callee::Func(f) if self.calls_body == Some(*f) => cx.body(*f, at)?,
            Callee::Func(f) => cx.plan(Address::Func(*f), at)?,
            Callee::Import(i) => cx.plan(Address::Import(*i), at)?,
            Callee::Indirect(_, signature) => {
                owned = cx.isa.plan(signature).map_err(|m| m.at(at))?;
                Some(&owned)
            }
        };
        let plan = plan.expect("what is called is a function");
        let (args, result) = self.arguments(plan, args);
        let call = match callee {
            Callee::Func(f) if self.calls_body == Some(*f) => {
                let target = self.body_ref(*f, plan);
                self.b.ins().call(target, &args)
            }
            Callee::Func(f) => {
                let target = self.func_ref(Address::Func(*f), at)?;
                self.b.ins().call(target, &args)
            }
            Callee::Import(i) => {
                let target = self.func_ref(Address::Import(*i), at)?;
                self.b.ins().call(target, &args)
            }
            Callee::Indirect(target, _) => {
                let signature = self.b.import_signature(plan.signature.clone());
                let target = self.get(*target);
                self.b.ins().call_indirect(signature, target, &args)
            }
        };
        if let (Returned::Parts(parts), Some(to)) = (&plan.returned, result) {
            let returned = self.b.inst_results(call).to_vec();
            self.store_parts(parts, to, &returned);
        }
        Ok(defines.then(|| self.b.inst_results(call)[0]))
    }

    /// Jumps to the body of `callee`, which returns to this function's caller: this function is a
    /// body too, and so has the tail convention (`Tails`), or the call is refused.
    pub(super) fn tail_call(&mut self, callee: &Callee, args: &[Value], at: OriginId) -> Result<(), Lack> {
        let Callee::Func(f) = callee else { return Err(Missing::TailCall.at(at)) };
        // What is passed by address must be where this function's caller left it, not in this frame.
        let received = self.f.entry().map(|entry| self.f.blocks[entry].params.as_slice()).unwrap_or_default();
        let signature = &self.cx.unit.functions[*f].signature;
        if signature.held().any(|held| !received.contains(&args[held.at])) {
            return Err(Missing::TailCall.at(at));
        }
        let body = self.cx.body(*f, at)?;
        let (Some(plan), CallConv::Tail) = (body, self.plan.signature.call_conv) else {
            return Err(Missing::TailCall.at(at));
        };
        let (args, _) = self.arguments(plan, args);
        let target = self.body_ref(*f, plan);
        self.b.ins().return_call(target, &args);
        self.tailed = true;
        Ok(())
    }

    /// The body of `f`, declared the first time it is named.
    fn body_ref(&mut self, f: FuncId, plan: &Plan) -> FuncRef {
        if let Some(&declared) = self.bodies.get(&f) {
            return declared;
        }
        let signature = self.b.import_signature(plan.signature.clone());
        let name = self.b.func.declare_imported_user_function(Body(f).name());
        let data =
            ExtFuncData { name: ExternalName::user(name), signature, colocated: true, patchable: false };
        let declared = self.b.import_function(data);
        self.bodies.insert(f, declared);
        declared
    }

    /// The function or import at `target`, declared the first time it is named.
    fn func_ref(&mut self, target: Address, at: OriginId) -> Result<FuncRef, Lack> {
        if let Some(&declared) = self.refs.get(&target) {
            return Ok(declared);
        }
        let signature = self.cx.plan(target, at)?.expect("what is called is a function").signature.clone();
        let signature = self.b.import_signature(signature);
        let name = self.b.func.declare_imported_user_function(target.name());
        let data = ExtFuncData {
            name: ExternalName::user(name),
            signature,
            colocated: Self::colocated(target),
            patchable: false,
        };
        let declared = self.b.import_function(data);
        self.refs.insert(target, declared);
        Ok(declared)
    }

    /// The symbol of data or an import, declared the first time it is named; per thread where
    /// `tls`.
    fn global(&mut self, target: Address, tls: bool) -> GlobalValue {
        if let Some(&declared) = self.globals.get(&(target, tls)) {
            return declared;
        }
        let name = self.b.func.declare_imported_user_function(target.name());
        let data = GlobalValueData::Symbol {
            name: ExternalName::user(name),
            offset: Imm64::new(0),
            colocated: Self::colocated(target),
            tls,
        };
        let declared = self.b.create_global_value(data);
        self.globals.insert((target, tls), declared);
        declared
    }

    /// Whether the object defines `target`, which its code then reaches without the global
    /// offset table or the procedure linkage table.
    fn colocated(target: Address) -> bool {
        !matches!(target, Address::Import(_))
    }

    /// The address of a function, a datum or an import.
    pub(super) fn address(&mut self, target: Address, at: OriginId) -> Result<Val, Lack> {
        let is_function = match target {
            Address::Func(_) => true,
            Address::Data(_) => false,
            Address::Import(i) => matches!(self.cx.unit.imports[i].kind, ImportKind::Func(_)),
        };
        if is_function {
            let declared = self.func_ref(target, at)?;
            Ok(self.b.ins().func_addr(self.ptr, declared))
        } else {
            let declared = self.global(target, false);
            Ok(self.b.ins().symbol_value(self.ptr, declared))
        }
    }

    /// The calling thread's storage of `data`.
    pub(super) fn thread_address(&mut self, data: DataId) -> Val {
        let declared = self.global(Address::Data(data), true);
        self.b.ins().tls_value(self.ptr, declared)
    }
}
