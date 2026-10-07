//! One LIR function as a Cranelift one: a block per reached block in `Order`, the block
//! parameters as Cranelift's, a stack slot per slot, and each instruction lowered where it is.

mod call;
mod cast;
mod copy;
mod inst;
mod record;
mod stub;
mod term;

use std::collections::HashMap;

use cranelift_codegen::ir::{
    self, FuncRef, GlobalValue, MemFlagsData, SourceLoc, StackSlotData, StackSlotKind, Type,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use vun_backend::{Lack, Missing};
use vun_index::IndexVec;
use vun_lir::analysis::{Cfg, Loops, Order};
use vun_lir::{Address, Block, FuncId, Function, ImportId, ImportKind, Slot, Unit, Value};
use vun_origin::OriginId;

use crate::abi::Plan;
use crate::isa::Isa;
use crate::names::{Body, Named};
use crate::plan::Parts;
use crate::tail::Tails;

/// How a signature is passed, or what Cranelift lacks of it: a call of a function it cannot
/// pass to is refused where it is, and the function holding the call is left to the assisting
/// backend.
type Passed = Result<Plan, Missing>;

/// What every function's lowering reads: the unit, the processor and the signature of each
/// function reached and import.
pub(crate) struct Cx<'a> {
    pub(crate) unit: &'a Unit,
    pub(crate) isa: &'a Isa,
    funcs: IndexVec<FuncId, Option<Passed>>,
    /// How the body of each function with an entry is passed to: in the tail convention.
    bodies: IndexVec<FuncId, Option<Passed>>,
    imports: IndexVec<ImportId, Option<Passed>>,
}

impl<'a> Cx<'a> {
    /// The context of `unit`, whose functions `reached` holds.
    pub(crate) fn new(
        unit: &'a Unit,
        isa: &'a Isa,
        reached: &vun_lir::analysis::Reached,
        tails: &Tails,
    ) -> Self {
        let funcs = unit.functions.iter().map(|(id, f)| reached.contains(id).then(|| isa.plan(&f.signature)));
        let bodies = unit.functions.iter().map(|(id, f)| {
            tails.contains(id).then(|| -> Passed {
                let mut plan = isa.plan(&f.signature)?;
                plan.signature.call_conv = CallConv::Tail;
                Ok(plan)
            })
        });
        let imports = unit.imports.iter().map(|(_, import)| match &import.kind {
            ImportKind::Func(signature) => Some(isa.plan(signature)),
            ImportKind::Data => None,
        });
        Self { unit, isa, funcs: funcs.collect(), bodies: bodies.collect(), imports: imports.collect() }
    }

    /// How the function or import at `address` is passed to, if it is one.
    ///
    /// # Errors
    /// Returns what Cranelift lacks of it, at `at`.
    pub(crate) fn plan(&self, address: Address, at: OriginId) -> Result<Option<&Plan>, Lack> {
        let passed = match address {
            Address::Func(f) => self.funcs[f].as_ref(),
            Address::Import(i) => self.imports[i].as_ref(),
            Address::Data(_) => None,
        };
        passed.map(|passed| passed.as_ref().map_err(|&m| m.at(at))).transpose()
    }
}

impl Cx<'_> {
    /// How the body of `f` is passed to, if it has an entry.
    ///
    /// # Errors
    /// Returns what Cranelift lacks of it, at `at`.
    pub(crate) fn body(&self, f: FuncId, at: OriginId) -> Result<Option<&Plan>, Lack> {
        self.bodies[f].as_ref().map(|passed| passed.as_ref().map_err(|&m| m.at(at))).transpose()
    }
}

/// A function lowered, with the origins its source locations number: its body in the tail
/// convention and the entry that calls it, if it has one (`Tails`), else the function itself.
#[derive(Clone)]
pub(crate) struct Lowered {
    pub(crate) func: ir::Function,
    pub(crate) entry: Option<ir::Function>,
    pub(crate) origins: Vec<OriginId>,
}

/// A function being lowered.
struct Lower<'a, 'b> {
    cx: &'a Cx<'a>,
    f: &'a Function,
    plan: &'a Plan,
    /// Where a record returned in registers is written, and what it is made of.
    returning: Option<(ir::Value, &'a Parts)>,
    b: FunctionBuilder<'b>,
    ptr: Type,
    values: IndexVec<Value, Option<ir::Value>>,
    blocks: IndexVec<Block, Option<ir::Block>>,
    slots: IndexVec<Slot, Option<ir::StackSlot>>,
    /// Whether the last instruction was a tail call, which ends its block.
    tailed: bool,
    refs: HashMap<Address, FuncRef>,
    bodies: HashMap<FuncId, FuncRef>,
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
        let body = self.body(id, f.origin)?;
        let plan = match body {
            Some(plan) => plan,
            None => self.plan(Address::Func(id), f.origin)?.expect("a function has a signature"),
        };
        let name = if body.is_some() { Body(id).name() } else { Address::Func(id).name() };
        let mut func =
            ir::Function::with_name_signature(ir::UserFuncName::User(name), plan.signature.clone());
        let mut context = FunctionBuilderContext::new();
        let mut lower = Lower {
            cx: self,
            f,
            plan,
            returning: None,
            b: FunctionBuilder::new(&mut func, &mut context),
            ptr: self.isa.target.pointer_type(),
            values: IndexVec::filled(None, f.values.len()),
            blocks: IndexVec::filled(None, f.blocks.len()),
            slots: IndexVec::filled(None, f.slots.len()),
            tailed: false,
            refs: HashMap::new(),
            bodies: HashMap::new(),
            globals: HashMap::new(),
            origins: Vec::new(),
        };
        lower.body()?;
        let mut origins = lower.origins;
        lower.b.seal_all_blocks();
        lower.b.finalize(self.isa.target.frontend_config());
        let entry = body.is_some().then(|| self.entry(id, f, &mut origins)).transpose()?;
        Ok(Lowered { func, entry, origins })
    }
}

impl Lower<'_, '_> {
    fn body(&mut self) -> Result<(), Lack> {
        let f = self.f;
        let entry = f.entry().expect("a function has a body");
        let order = Order::of(f, &Loops::of(f, &Cfg::of(f)));
        let mut passed = Vec::new();
        for &block in order.blocks() {
            let made = self.b.create_block();
            if block == entry {
                self.b.append_block_params_for_function_params(made);
                passed = self.b.block_params(made).to_vec();
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
        self.bind(&f.blocks[entry].params, passed);
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
