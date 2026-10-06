//! The Cranelift backend (spec 13, section 8.5): machine code for debug builds, generated in
//! process. Each function of a unit becomes a Cranelift function (`func`), compiled on its own
//! (`compile`); the code and its relocations are placed in an object as the native backend's are
//! (`vun_codegen::Assembly`).

mod abi;
mod compile;
mod failed;
mod func;
mod isa;
mod listing;
mod names;
mod relocs;
#[cfg(test)]
mod tests;

use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Feature, HeldFloats, Part, Readiness,
    Request, Widest,
};
use vun_codegen::Assembly;
use vun_diag::{DiagCtxt, ErrorGuaranteed};
use vun_lir_passes::legalize::Half;
use vun_target::{Name, Names};

use compile::Compilation;
use isa::Isa;
use listing::Listing;

pub const INFO: BackendInfo = BackendInfo {
    id: Name("cranelift"),
    arches: Names::Only(isa::NAMES),
    formats: Names::Only(&isa::FORMAT_NAMES),
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[Feature::DebugInfo, Feature::FrameSizes],
        widest: Widest::Bits(64),
        half: Half::Promoted,
        floats: HeldFloats::Every,
    },
    readiness: Readiness::Ready,
    runs: &[],
};

/// The backend. What it cannot express (inline assembly and naked functions) goes to the
/// request's assist: without one for the target, a program holding them is refused.
pub struct CraneliftBackend;

impl Backend for CraneliftBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let isa = Isa::of(request.target, request.profile)
            .map_err(|missing| diag.error(missing.diagnostic(INFO.id)))?;
        let assisted = request.assist.assists(request.target);
        let handover = |ids: &[_]| {
            for &id in ids {
                let name = request.lines.name(request.unit.functions[id].origin);
                vun_trace::tracing::debug!(id = vun_index::Idx::index(id), ?name, "handed over");
            }
            request.assist.compile(request, ids)
        };
        let compiled =
            Compilation::of(request.unit, &isa, assisted.then_some(&handover as _), request.listing)
                .map_err(|failed| diag.error(failed.diagnostic(INFO.id)))?;
        let support: Vec<&str> = compiled.support.iter().map(|s| s.symbol()).collect();
        let assembly = Assembly {
            unit: request.unit,
            code: &compiled.code,
            lines: request.lines,
            frames: None,
            support: &support,
        };
        let bytes = assembly.object((isa.machine, isa.format)).write();
        out.put(Artifact { part: Part::Whole, kind: Emits::Object, bytes });
        let word = u64::from(request.unit.pointer_bits / u8::BITS);
        out.frames(compiled.code.iter().map(|(id, c)| (id, c.size(word))).collect());
        if request.listing {
            let text = Listing { unit: request.unit, functions: &compiled.listing, lines: request.lines };
            let text = text.to_string();
            out.put(Artifact { part: Part::Whole, kind: Emits::Listing, bytes: text.into_bytes() });
        }
        Ok(())
    }
}
