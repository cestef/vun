//! The entry of a function compiled in the tail convention (`Tails`): a function of the target's
//! own convention that calls the body, so that every other caller, and every address, reaches the
//! function as the target's convention passes to it. Wasmtime's trampolines do the same where its
//! tail-convention functions meet the host.

use cranelift_codegen::ir::{self, ExtFuncData, ExternalName, InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use vun_backend::Lack;
use vun_lir::{Address, FuncId, Function};
use vun_origin::OriginId;

use super::Cx;
use crate::names::{Body, Named};

impl Cx<'_> {
    /// The entry of the function `id`, `f`, whose body is lowered: its origin is among `origins`,
    /// which the source locations of the body number.
    ///
    /// # Errors
    /// Returns what Cranelift lacks of the signature.
    pub(super) fn entry(
        &self,
        id: FuncId,
        f: &Function,
        origins: &mut Vec<OriginId>,
    ) -> Result<ir::Function, Lack> {
        let plan = self.plan(Address::Func(id), f.origin)?.expect("a function has a signature");
        let body = self.body(id, f.origin)?.expect("a function with an entry has a body");
        let name = ir::UserFuncName::User(Address::Func(id).name());
        let mut func = ir::Function::with_name_signature(name, plan.signature.clone());
        let mut context = FunctionBuilderContext::new();
        let mut b = FunctionBuilder::new(&mut func, &mut context);
        let block = b.create_block();
        b.append_block_params_for_function_params(block);
        b.switch_to_block(block);
        let at = origins.iter().position(|&o| o == f.origin).unwrap_or_else(|| {
            origins.push(f.origin);
            origins.len() - 1
        });
        b.set_srcloc(ir::SourceLoc::new(u32::try_from(at).expect("origins fit u32")));
        let params = b.block_params(block).to_vec();
        let signature = b.import_signature(body.signature.clone());
        let callee = b.func.declare_imported_user_function(Body(id).name());
        let callee = b.import_function(ExtFuncData {
            name: ExternalName::user(callee),
            signature,
            colocated: true,
            patchable: false,
        });
        let call = b.ins().call(callee, &params);
        let results = b.inst_results(call).to_vec();
        b.ins().return_(&results);
        b.seal_all_blocks();
        b.finalize(self.isa.target.frontend_config());
        Ok(func)
    }
}
