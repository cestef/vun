//! One LIR function as a Cranelift one: a block per reached block in `Order`, the block
//! parameters as Cranelift's, a stack slot per slot, and each instruction lowered where it is.

mod call;
mod cast;
mod copy;
mod inst;
mod term;

use std::collections::HashMap;

use cranelift_codegen::ir::{
    self, FuncRef, GlobalValue, MemFlagsData, SourceLoc, StackSlotData, StackSlotKind, Type,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use vun_backend::{Lack, Missing};
use vun_index::{Idx, IndexVec};
use vun_lir::analysis::{Cfg, Loops, Order};
use vun_lir::{Address, Block, FuncId, Function, ImportId, ImportKind, Slot, Unit, Value};
use vun_origin::OriginId;

use crate::isa::Isa;

/// What a signature is passed as, or what Cranelift lacks of it: a call of a function it cannot
/// pass to is refused where it is, and the function holding the call is left to the assisting
/// backend.
type Passed = Result<ir::Signature, Missing>;

/// What every function's lowering reads: the unit, the processor and the signature of each
/// function reached and import.
pub(crate) struct Cx<'a> {
    pub(crate) unit: &'a Unit,
    pub(crate) isa: &'a Isa,
    funcs: IndexVec<FuncId, Option<Passed>>,
    imports: IndexVec<ImportId, Option<Passed>>,
}

impl<'a> Cx<'a> {
    /// The context of `unit`, whose functions `reached` holds.
    pub(crate) fn new(unit: &'a Unit, isa: &'a Isa, reached: &vun_lir::analysis::Reached) -> Self {
        let funcs =
            unit.functions.iter().map(|(id, f)| reached.contains(id).then(|| isa.signature(&f.signature)));
        let imports = unit.imports.iter().map(|(_, import)| match &import.kind {
            ImportKind::Func(signature) => Some(isa.signature(signature)),
            ImportKind::Data => None,
        });
        Self { unit, isa, funcs: funcs.collect(), imports: imports.collect() }
    }

    /// The signature of the function or import at `address`, if it is one.
    ///
    /// # Errors
    /// Returns what Cranelift lacks of it, at `at`.
    pub(crate) fn signature(&self, address: Address, at: OriginId) -> Result<Option<&ir::Signature>, Lack> {
        let passed = match address {
            Address::Func(f) => self.funcs[f].as_ref(),
            Address::Import(i) => self.imports[i].as_ref(),
            Address::Data(_) => None,
        };
        passed.map(|passed| passed.as_ref().map_err(|&m| m.at(at))).transpose()
    }
}

/// A function lowered, with the origins its source locations number.
pub(crate) struct Lowered {
    pub(crate) func: ir::Function,
    pub(crate) origins: Vec<OriginId>,
}

/// A function being lowered.
struct Lower<'a, 'b> {
    cx: &'a Cx<'a>,
    f: &'a Function,
    b: FunctionBuilder<'b>,
    ptr: Type,
    values: IndexVec<Value, Option<ir::Value>>,
    blocks: IndexVec<Block, Option<ir::Block>>,
    slots: IndexVec<Slot, Option<ir::StackSlot>>,
    refs: HashMap<Address, FuncRef>,
    globals: HashMap<(Address, bool), GlobalValue>,
    origins: Vec<OriginId>,
}

impl Cx<'_> {
    /// `f`, the function `id`, as Cranelift's.
    ///
    /// # Errors
    /// Returns the first thing it holds that Cranelift does not express.
    ///
    /// # Panics
    /// If `id` is not reached.
    pub(crate) fn lower(&self, id: FuncId, f: &Function) -> Result<Lowered, Lack> {
        let signature = self.signature(Address::Func(id), f.origin)?;
        let signature = signature.expect("a function has a signature").clone();
        let mut func = ir::Function::with_name_signature(
            ir::UserFuncName::user(0, id.index().try_into().expect("ids fit u32")),
            signature,
        );
        let mut context = FunctionBuilderContext::new();
        let mut lower = Lower {
            cx: self,
            f,
            b: FunctionBuilder::new(&mut func, &mut context),
            ptr: self.isa.target.pointer_type(),
            values: IndexVec::filled(None, f.values.len()),
            blocks: IndexVec::filled(None, f.blocks.len()),
            slots: IndexVec::filled(None, f.slots.len()),
            refs: HashMap::new(),
            globals: HashMap::new(),
            origins: Vec::new(),
        };
        lower.body()?;
        let origins = lower.origins;
        lower.b.seal_all_blocks();
        lower.b.finalize(self.isa.target.frontend_config());
        Ok(Lowered { func, origins })
    }
}

impl Lower<'_, '_> {
    fn body(&mut self) -> Result<(), Lack> {
        let f = self.f;
        let entry = f.entry().expect("a function has a body");
        let order = Order::of(f, &Loops::of(f, &Cfg::of(f)));
        for &block in order.blocks() {
            let made = self.b.create_block();
            if block == entry {
                self.b.append_block_params_for_function_params(made);
                let passed = self.b.block_params(made).to_vec();
                for (&param, value) in f.blocks[block].params.iter().zip(passed) {
                    self.values[param] = Some(value);
                }
            } else {
                for &param in &f.blocks[block].params {
                    let ty = self.ty(f.ty(param), f.origin)?;
                    self.values[param] = Some(self.b.append_block_param(made, ty));
                }
            }
            self.blocks[block] = Some(made);
        }
        self.b.switch_to_block(self.blocks[entry].expect("the entry is reached"));
        for (slot, data) in f.slots.iter() {
            let shift = u8::try_from(data.align.max(1).trailing_zeros()).expect("alignments are powers of 2");
            let size = u32::try_from(data.size).expect("slots fit u32");
            let made =
                self.b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, size, shift));
            self.slots[slot] = Some(made);
        }
        for &block in order.blocks() {
            if block != entry {
                self.b.switch_to_block(self.blocks[block].expect("made above"));
            }
            for &inst in &f.blocks[block].insts {
                self.inst(inst)?;
            }
            self.terminate(&f.blocks[block]);
        }
        Ok(())
    }

    /// The type of `scalar`, or what it lacks, at `origin`.
    fn ty(&self, scalar: vun_lir::Scalar, origin: OriginId) -> Result<Type, Lack> {
        self.cx.isa.ty(scalar).map_err(|m| m.at(origin))
    }

    /// The Cranelift value of `value`.
    fn get(&self, value: Value) -> ir::Value {
        self.values[value].expect("a value is defined before it is read")
    }

    /// The flags of an access that assumes nothing of its address.
    const fn plain() -> MemFlagsData {
        MemFlagsData::new()
    }

    fn get_all(&self, values: &[Value]) -> Vec<ir::Value> {
        values.iter().map(|&v| self.get(v)).collect()
    }

    /// Records where the code that follows comes from.
    fn locate(&mut self, origin: OriginId) {
        let at = self.origins.iter().position(|&o| o == origin).unwrap_or_else(|| {
            self.origins.push(origin);
            self.origins.len() - 1
        });
        self.b.set_srcloc(SourceLoc::new(u32::try_from(at).expect("origins fit u32")));
    }
}
