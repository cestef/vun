//! The wasm backend (spec 13, section 8.4): LIR to a relocatable WebAssembly object in the tool
//! conventions' format, which `wasm-ld` links. The module is typed data (`module`), verified
//! before it is written; `build` makes it from a unit.

mod build;
mod listing;
pub mod module;

#[cfg(test)]
mod tests;

use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Missing, Part, Readiness, Request,
};
use vun_diag::{DiagCtxt, ErrorGuaranteed};

pub use listing::Listing;
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("wasm"),
    arches: Arches::Only(&[Name("wasm32")]),
    caps: BackendCaps { emits: Emits::Object, features: &[], int_bits: 64, half: Half::Promoted },
    readiness: Readiness::Ready,
};

/// The object format the backend writes, as targets name it.
const FORMAT: Name = Name("wasm");

#[derive(Default)]
pub struct WasmBackend;

impl Backend for WasmBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        if !FORMAT.is(request.target.object_format) {
            return Err(diag.error(Missing::Format(request.target.object_format).diagnostic(INFO.id)));
        }
        let module = build::module(request.unit, request.lines)
            .map_err(|lack| diag.error(lack.diagnostic(INFO.id)))?;
        if let Err(invalid) = module.verify() {
            panic!("the wasm backend made an invalid module: {invalid}");
        }
        out.put(Artifact { part: Part::Whole, kind: Emits::Object, bytes: module.write() });
        if request.listing {
            let text = Listing { module: &module, lines: request.lines }.to_string();
            out.put(Artifact { part: Part::Whole, kind: Emits::Listing, bytes: text.into_bytes() });
        }
        Ok(())
    }
}
