//! Each function with its small callees inlined (spec 13, section 3): simplifying's second phase,
//! done per function as its first is (`piece`), so that a library function's is lowered once and
//! shared as its LIR is. A callee inlined, itself inlined already, is placed in the caller's
//! tables (`placing`), the checks applying in it as they do in it wherever it is: a function the
//! checks run in stays unchecked (`unchecked`), which is why the functions they run in key it. A
//! function reached again through its callees inlines none of theirs.

use std::sync::Arc;

use vun_codec::{Decode, Encode};
use vun_hash::Map;
use vun_index::Idx;
use vun_lir::{Address, DataId, FuncId, Function, ImportId, Site};
use vun_lir_passes::{Hoist, InlineCalls, Tidy};
use vun_origin::{OriginId, OriginTable, Via};
use vun_query::{Cx, Cycle, Query};
use vun_sema::target::TargetOf;
use vun_sema::trace::TraceOf;
use vun_sema_ty::TyKind;
use vun_sema_ty::item::DeclRef;
use vun_target::Detail;

use crate::piece::{Datum, DatumKind, FuncRef, FunctionLir, Lir, Piece, Refs};
use crate::placing::{Places, SiteIds};
use crate::repr::Types;
use crate::unchecked::Unchecked;
use crate::unlowered::{Unlowered, Why};

/// A function of a program, and the functions of the program the checks run in.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Inlining {
    pub piece: Piece,
    pub unchecked: Unchecked,
}

/// Each function of a program with its small callees inlined, and tidied again.
pub struct InlinedLir;

impl Query for InlinedLir {
    const NAME: &'static str = "inlined_lir";
    type Key = Inlining;
    type Value = Arc<Result<Lir, Unlowered>>;

    fn compute(cx: &Cx<'_>, inlining: &Inlining) -> Self::Value {
        Arc::new(inlining.inlined(cx))
    }

    fn cycle(cx: &Cx<'_>, inlining: &Inlining, _: &Cycle) -> Self::Value {
        cx.get::<FunctionLir>(&inlining.piece)
    }
}

impl Inlining {
    /// The function `named`, which this one calls.
    pub(crate) fn callee(&self, named: &FuncRef) -> Option<Self> {
        let piece = match named {
            FuncRef::Function(f) => Piece::of(f, false, &self.unchecked),
            FuncRef::End(f) => Piece::of(f, true, &self.unchecked),
            FuncRef::Constructor(ty, name) => {
                let TyKind::Nominal(nominal) = &**ty else { return None };
                Piece::Constructor { nominal: nominal.clone(), name: *name }
            }
        };
        Some(Self { piece, unchecked: self.unchecked.clone() })
    }

    /// Its LIR with the small functions it calls inlined, tidied again, and the addresses its
    /// loops use made before them.
    fn inlined(&self, cx: &Cx<'_>) -> Result<Lir, Unlowered> {
        let mut lir = self.merged(cx)?;
        lir.function.hoist_addresses();
        Ok(lir)
    }

    fn merged(&self, cx: &Cx<'_>) -> Result<Lir, Unlowered> {
        let mut lir = cx.get::<FunctionLir>(&self.piece).as_ref().clone()?;
        let mut called: Vec<FuncId> = lir.function.callees().collect();
        called.sort_unstable();
        called.dedup();
        let named: Vec<(FuncId, Self)> = called
            .into_iter()
            .filter_map(|g| Some((g, self.callee(&lir.refs.functions[g.index()])?)))
            .collect();
        let own: Vec<FuncId> = named.iter().filter(|(_, callee)| callee == self).map(|&(g, _)| g).collect();
        let callees: Vec<(FuncId, Self)> = named.into_iter().filter(|(_, callee)| callee != self).collect();
        let keys: Vec<Self> = callees.iter().map(|(_, callee)| callee.clone()).collect();
        let bodies = cx.get_all::<InlinedLir>(&keys);
        let at = lir.function.origin;
        let mut merging = Merging::new(&mut lir.refs, at);
        // Where the table describes calls, a callee is inlined only if no call it makes returns
        // into it: a return address then lies in the code of the function it names.
        let leaves = cx.get::<TraceOf>(&()) == Detail::Calls;
        let allowed = |function: &Function| function.small() && !(leaves && function.returning_calls());
        let mut small = Vec::new();
        for ((g, _), body) in callees.iter().zip(&bodies) {
            if let Ok(body) = body.as_ref()
                && allowed(&body.function)
            {
                small.push((*g, merging.place(body, at)?));
            }
        }
        if small.is_empty() && own.is_empty() {
            return Ok(lir);
        }
        lir.function.inline_small(small.iter().map(|(g, body)| (*g, body)));
        let target = cx.get::<TargetOf>(&()).ok_or(Unlowered::new(at, Why::Untargeted))?;
        let types = Types::new(cx, &target).map_err(|bits| Unlowered::new(at, Why::Address(bits)))?;
        let bits = types.pointer_bits();
        // A function calling itself is inlined into itself again, tidied between, while it stays
        // small: a call then does the work of several of the recursion.
        lir.function.tidy(bits);
        for _ in 0..UNROLLED {
            let body = lir.function.clone();
            if !allowed(&body) || !own.iter().any(|g| body.callees().any(|c| c == *g)) {
                break;
            }
            lir.function.inline_small(own.iter().map(|&g| (g, &body)));
            lir.function.tidy(bits);
        }
        Ok(lir)
    }
}

/// How many times a function that calls itself is inlined into itself.
const UNROLLED: usize = 3;

/// A function's own tables, taking the ids of the callees inlined into it: each function, link
/// declaration and top-level binding once, the constants and trap sites of each callee after
/// the function's.
struct Merging<'r> {
    refs: &'r mut Refs,
    functions: Map<FuncRef, FuncId>,
    imports: Map<DeclRef, ImportId>,
    globals: Map<DeclRef, DataId>,
    sites: SiteIds,
    /// The function the callees are inlined into.
    into: OriginId,
}

impl<'r> Merging<'r> {
    fn new(refs: &'r mut Refs, into: OriginId) -> Self {
        let functions = refs.functions.iter().cloned().zip((0..).map(FuncId::new)).collect();
        let imports = refs.imports.iter().cloned().zip((0..).map(ImportId::new)).collect();
        let globals = (refs.data.iter().zip((0..).map(DataId::new)))
            .filter_map(|(datum, id)| match &datum.kind {
                DatumKind::Global { decl, .. } => Some((decl.clone(), id)),
                DatumKind::Constant => None,
            })
            .collect();
        let sites = SiteIds::of(&refs.sites);
        Self { refs, functions, imports, globals, sites, into }
    }
}

impl Places for Merging<'_> {
    fn function(&mut self, named: &FuncRef, _: OriginId) -> Result<Address, Unlowered> {
        let functions = &mut self.refs.functions;
        let id = *self.functions.entry(named.clone()).or_insert_with(|| {
            functions.push(named.clone());
            FuncId::new(functions.len() - 1)
        });
        Ok(Address::Func(id))
    }

    fn linked(&mut self, decl: &DeclRef, _: OriginId) -> Result<Address, Unlowered> {
        let imports = &mut self.refs.imports;
        let id = *self.imports.entry(decl.clone()).or_insert_with(|| {
            imports.push(decl.clone());
            ImportId::new(imports.len() - 1)
        });
        Ok(Address::Import(id))
    }

    fn datum(&mut self, datum: Datum) -> DataId {
        if let DatumKind::Global { decl, .. } = &datum.kind
            && let Some(&id) = self.globals.get(decl)
        {
            return id;
        }
        let global = match &datum.kind {
            DatumKind::Global { decl, .. } => Some(decl.clone()),
            DatumKind::Constant => None,
        };
        self.refs.data.push(datum);
        let id = DataId::new(self.refs.data.len() - 1);
        if let Some(decl) = global {
            self.globals.insert(decl, id);
        }
        id
    }

    /// The callee's sites, each where it is inlined: into the function at `self.into`.
    fn sites(&mut self, sites: &[Site], at: OriginId) -> Result<Vec<u32>, Unlowered> {
        let origins = OriginTable::global();
        let inlined = |site: &Site| {
            let span = origins.get(site.at).span;
            Site { at: origins.derive(site.at, span, Via::Inline { into: self.into }), msg: site.msg.clone() }
        };
        sites.iter().map(|site| self.sites.id(&mut self.refs.sites, &inlined(site), at)).collect()
    }
}
