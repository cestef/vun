//! Calls, and the addresses of functions, data and storage per thread.

use cranelift_codegen::ir::immediates::Imm64;
use cranelift_codegen::ir::{
    ExtFuncData, ExternalName, FuncRef, GlobalValue, GlobalValueData, InstBuilder, Value as Val,
};
use vun_backend::Lack;
use vun_lir::{Address, Callee, DataId, ImportKind, Value};
use vun_origin::OriginId;

use super::Lower;
use crate::names::Named;

impl Lower<'_, '_> {
    /// Calls `callee`, giving its result if `defines` says the instruction has one.
    pub(super) fn call(
        &mut self,
        callee: &Callee,
        args: &[Value],
        defines: bool,
        at: OriginId,
    ) -> Result<Option<Val>, Lack> {
        let args = self.get_all(args);
        let call = match callee {
            Callee::Func(f) => {
                let target = self.func_ref(Address::Func(*f), at)?;
                self.b.ins().call(target, &args)
            }
            Callee::Import(i) => {
                let target = self.func_ref(Address::Import(*i), at)?;
                self.b.ins().call(target, &args)
            }
            Callee::Indirect(target, signature) => {
                let signature = self.cx.isa.signature(signature).map_err(|m| m.at(at))?;
                let signature = self.b.import_signature(signature);
                let target = self.get(*target);
                self.b.ins().call_indirect(signature, target, &args)
            }
        };
        Ok(defines.then(|| self.b.inst_results(call)[0]))
    }

    /// The function or import at `target`, declared the first time it is named.
    fn func_ref(&mut self, target: Address, at: OriginId) -> Result<FuncRef, Lack> {
        if let Some(&declared) = self.refs.get(&target) {
            return Ok(declared);
        }
        let signature = self.cx.signature(target, at)?.expect("what is called is a function").clone();
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
