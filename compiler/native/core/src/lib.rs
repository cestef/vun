//! The native backend's shared layer (spec 13, section 8.3): what every architecture's
//! instruction compilers are driven through, over `vun-codegen`, the assemblers `vun-asm-*` and
//! the object writer `vun-object`. Each architecture is a crate of its own implementing
//! [`Native`]; `vun-backend-native` lists them.

pub mod inline;
mod listing;
pub mod select;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod width;

use std::fmt::Display;

use vun_asm::Isa;
use vun_backend::Compiled;
use vun_backend::{Artifact, Emits, Emitted, Kept, Lack, Part, Request};
use vun_codegen::{Assembly, Isel, Processor};
use vun_index::{Idx, IndexVec};
use vun_lir::analysis::Reached;
use vun_lir::unit::Unit;
use vun_lir::{Address, FuncId, Function};
use vun_lir_passes::legalize::Division;
use vun_object::{Format, Machine};
use vun_par::Parallel;
use vun_target::TargetSpec;
use vun_trace::Activity;

pub use listing::Listing;

/// A function compiled for the architecture `A`.
const COMPILE: Activity = Activity::stage("native compile functions");
const WRITE: Activity = Activity::stage("native write object");

pub type Code<A> = Compiled<<<A as vun_codegen::Arch>::Isa as Isa>::Inst<Address>>;

/// An architecture's instruction compilers, with the calling conventions it compiles for.
pub trait Native: Isel {
    /// How its call frames are described to unwinders.
    const CALL_FRAME: vun_dwarf::CallFrame;
    /// The conventions it follows, by the name targets give them.
    const CONVENTIONS: &'static [(vun_target::Convention, Self::Convention)];

    /// The machine of `target`'s objects.
    fn machine(target: &TargetSpec) -> Machine;

    /// Whether `processor` has the architecture's division instructions at the width of its
    /// registers, which the widest integer is.
    fn divides(processor: Processor<Self>) -> bool;

    /// The processor of `target`: the features of the architecture's instruction set it names,
    /// and the floats it computes.
    #[must_use]
    fn processor(target: &TargetSpec) -> Processor<Self> {
        Processor::<Self>::of(&target.features, target.floats)
    }

    /// How integers are divided on `target`: by the processor's instructions, or through
    /// legalization's helper where it has none.
    #[must_use]
    fn division(target: &TargetSpec) -> Division {
        if Self::divides(Self::processor(target)) { Division::Computed } else { Division::Restoring }
    }

    /// The convention `target` follows, if the architecture compiles for it.
    fn convention(target: &TargetSpec) -> Option<Self::Convention> {
        Self::CONVENTIONS.iter().find(|(name, _)| *name == target.convention).map(|&(_, c)| c)
    }

    /// The machine code of the functions of `unit` that `chosen` says, for `processor`, compiled in
    /// parallel, listed when `listed`; the others have none. Unlisted code is taken from `kept`
    /// where it holds it (decision 669), and kept there when compiled.
    ///
    /// # Errors
    /// Returns the first thing a function needs that the backend cannot compile yet, or that the
    /// processor lacks.
    fn compile(
        unit: &Unit,
        convention: Self::Convention,
        processor: Processor<Self>,
        listed: bool,
        chosen: &(dyn Fn(FuncId) -> bool + Sync),
        kept: Option<&dyn Kept>,
    ) -> Result<IndexVec<FuncId, Code<Self>>, Lack>
    where
        Code<Self>: Send,
    {
        // A function inlined everywhere it was called is left to nothing: it has no code.
        let reached = Reached::of(unit);
        let functions: Vec<(FuncId, &Function)> = unit.functions.iter().collect();
        let kept = kept.filter(|_| !listed);
        let compiling = COMPILE.start();
        let compiled = functions.par_map(|&(id, f)| {
            if !reached.contains(id) || !chosen(id) {
                return Ok(Compiled::unreached());
            }
            let _span = vun_trace::tracing::debug_span!("native", f = %f.linkage).entered();
            if let Some(code) = kept.and_then(|kept| kept.code(id)) {
                return Ok(code.unlisted());
            }
            let compiled = vun_codegen::compile::<Self>(unit, f, convention, processor, listed)?;
            vun_trace::tracing::debug!(bytes = compiled.code.bytes.len(), frame = compiled.frame, "compiled");
            if let Some(kept) = kept {
                kept.keep(id, compiled.to_unlisted());
            }
            Ok(compiled)
        });
        drop(compiling);
        Ok(compiled.into_iter().collect::<Result<Vec<_>, _>>()?.into())
    }

    /// The code of the functions `ids` of `request`'s unit, as another backend's object holds it.
    ///
    /// # Errors
    /// As [`Self::compile`].
    ///
    /// # Panics
    /// For a target whose convention this does not follow, which assisting checks first.
    fn assist(request: &Request<'_>, ids: &[FuncId]) -> Result<Vec<(FuncId, Compiled<()>)>, Lack>
    where
        Code<Self>: Send,
    {
        let convention = Self::convention(request.target).expect("checked before assisting");
        let processor = Self::processor(request.target);
        let chosen = |id| ids.contains(&id);
        let code = Self::compile(request.unit, convention, processor, false, &chosen, request.kept)?;
        let code = code.into_vec().into_iter().enumerate();
        Ok(code.map(|(i, c)| (FuncId::new(i), c.unlisted())).filter(|(id, _)| ids.contains(id)).collect())
    }

    /// The relocatable object of `request`'s unit, each function's frame size (with the return
    /// address and saved frame pointer, as C compilers count them), and its listing if asked.
    ///
    /// # Errors
    /// As [`Self::compile`].
    ///
    /// # Panics
    /// For a target whose convention this does not follow, which emitting checks first.
    fn object(request: &Request<'_>, machine: Machine, format: Format) -> Result<Emitted, Lack>
    where
        Code<Self>: Send,
        <Self::Isa as Isa>::Inst<Address>: Display,
    {
        let convention = Self::convention(request.target).expect("checked before emitting");
        let code = Self::compile(
            request.unit,
            convention,
            Self::processor(request.target),
            request.listing,
            &|_| true,
            request.kept,
        )?;
        let writing = WRITE.start();
        let bytes = Assembly {
            unit: request.unit,
            code: &code,
            lines: request.lines,
            frames: Some(&Self::CALL_FRAME),
            support: &[],
        }
        .object((machine, format))
        .write();
        drop(writing);
        let mut artifacts = vec![Artifact { part: Part::Whole, kind: Emits::Object, bytes }];
        if request.listing {
            let text = Listing { unit: request.unit, code: &code, lines: request.lines }.to_string();
            artifacts.push(Artifact { part: Part::Whole, kind: Emits::Listing, bytes: text.into_bytes() });
        }
        let frames = code.iter().map(|(id, c)| (id, c.size(Self::WORD))).collect();
        Ok(Emitted { artifacts, frames })
    }
}
