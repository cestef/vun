//! The native backend's shared layer (spec 13, section 8.3): what every architecture's
//! instruction compilers are driven through, over `vun-codegen`, the assemblers `vun-asm-*` and
//! the object writer `vun-object`. Each architecture is a crate of its own implementing
//! [`Native`]; `vun-backend-native` lists them.

pub mod inline;
mod listing;
mod object;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod width;

use std::fmt::Display;

use vun_asm::Isa;
use vun_backend::{Artifact, Emits, Emitted, Lack, Part, Request};
use vun_codegen::{Compiled, Isel};
use vun_index::IndexVec;
use vun_lir::analysis::Reached;
use vun_lir::unit::Unit;
use vun_lir::{Address, FuncId, Function};
use vun_object::{Format, Machine};
use vun_par::Parallel;
use vun_target::TargetSpec;

use object::Assembly;

pub use listing::Listing;

/// A function compiled for the architecture `A`.
pub type Code<A> = Compiled<<<A as vun_codegen::Arch>::Isa as Isa>::Inst<Address>>;

/// An architecture's instruction compilers, with the calling conventions it compiles for.
pub trait Native: Isel {
    /// How its call frames are described to unwinders.
    const CALL_FRAME: vun_dwarf::CallFrame;
    /// The conventions it follows, by the name targets give them.
    const CONVENTIONS: &'static [(vun_target::Convention, Self::Convention)];

    /// The convention `target` follows, if the architecture compiles for it.
    fn convention(target: &TargetSpec) -> Option<Self::Convention> {
        Self::CONVENTIONS.iter().find(|(name, _)| *name == target.convention).map(|&(_, c)| c)
    }

    /// The machine code of every function of `unit`, compiled in parallel, listed when `listed`.
    ///
    /// # Errors
    /// Returns the first thing a function needs that the backend cannot compile yet.
    fn compile(
        unit: &Unit,
        convention: Self::Convention,
        listed: bool,
    ) -> Result<IndexVec<FuncId, Code<Self>>, Lack>
    where
        Code<Self>: Send,
    {
        // A function inlined everywhere it was called is left to nothing: it has no code.
        let reached = Reached::of(unit);
        let functions: Vec<(FuncId, &Function)> = unit.functions.iter().collect();
        let compiled = functions.par_map(|&(id, f)| {
            if !reached.contains(id) {
                return Ok(Compiled::unreached());
            }
            let _span = vun_trace::tracing::debug_span!("native", f = %f.linkage).entered();
            let compiled = vun_codegen::compile::<Self>(unit, f, convention, listed);
            if let Ok(c) = &compiled {
                vun_trace::tracing::debug!(bytes = c.code.bytes.len(), frame = c.frame, "compiled");
            }
            compiled
        });
        Ok(compiled.into_iter().collect::<Result<Vec<_>, _>>()?.into())
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
        let code = Self::compile(request.unit, convention, request.listing)?;
        let bytes =
            Assembly { unit: request.unit, code: &code, lines: request.lines, frames: &Self::CALL_FRAME }
                .object((machine, format))
                .write();
        let mut artifacts = vec![Artifact { part: Part::Whole, kind: Emits::Object, bytes }];
        if request.listing {
            let text = Listing { unit: request.unit, code: &code, lines: request.lines }.to_string();
            artifacts.push(Artifact { part: Part::Whole, kind: Emits::Listing, bytes: text.into_bytes() });
        }
        // Above the frame, a call leaves the return address and the saved frame pointer.
        let linkage = 2 * Self::WORD;
        let frames = code.iter().map(|(id, c)| (id, c.frame + linkage)).collect();
        Ok(Emitted { artifacts, frames })
    }
}
