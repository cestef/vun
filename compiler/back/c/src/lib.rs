//! Strict C99 through the target's C compiler: reaches every target with one (spec 13, section
//! 8.2). The backend builds C as data (`ast`) from LIR (`emit`) and prints it once.

pub mod ast;
mod emit;

pub use vun_backend::{Lack, Missing, Readiness};
use vun_lir_passes::legalize::Half;

use std::collections::{BTreeMap, HashMap};
use vun_backend::{
    Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Feature, HeldFloats, Lines, Request,
    Widest,
};
use vun_diag::{DiagCtxt, ErrorGuaranteed};

use vun_lir::FuncId;
use vun_lir::unit::Unit;
use vun_target::{Arches, Name, TargetSpec};

use emit::Emitter;

pub const INFO: BackendInfo = BackendInfo {
    id: Name("c"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Source,
        features: &[Feature::CAbi, Feature::NoAlias, Feature::FrameSizes, Feature::DebugInfo],
        widest: Widest::Bits(64),
        half: Half::Promoted,
        floats: HeldFloats::Every,
    },
    readiness: Readiness::Ready,
};

/// The functions a shard of a program holds on average (spec 13, section 2): a function goes to
/// the shard its content hash gives, so shards are not filled evenly.
pub const SHARD: usize = 256;

pub struct CBackend {
    /// The functions a file holds on average; a larger program spans several.
    pub shard: usize,
}

impl Default for CBackend {
    fn default() -> Self {
        Self { shard: SHARD }
    }
}

impl CBackend {
    /// The C files of `unit` for `target`: their names, whether others include them, and their
    /// text.
    ///
    /// # Errors
    /// Returns what the unit needs that C99 cannot express.
    pub fn sources(
        &self,
        unit: &Unit,
        target: &TargetSpec,
        lines: &dyn Lines,
    ) -> Result<Vec<Artifact>, Lack> {
        let sources = Emitter::sources(unit, target, lines, self.shard)?;
        Ok(sources
            .into_iter()
            .map(|s| Artifact { part: s.part, kind: s.kind(), bytes: s.file.to_string().into_bytes() })
            .collect())
    }
}

impl Backend for CBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        request: &Request<'_>,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let artifacts = self
            .sources(request.unit, request.target, request.lines)
            .map_err(|lack| diag.error(lack.diagnostic(INFO.id)))?;
        for artifact in artifacts {
            out.put(artifact);
        }
        Ok(())
    }

    /// From the `.su` files `-fstack-usage` writes: a line per function, `file:line:col:name`,
    /// its frame's bytes, and how they are known, separated by tabs. GCC names the clones it
    /// makes `name.suffix`: a `.part` is split out of its function and called from it, so its
    /// frame adds; any other clone replaces its function, so the larger frame counts.
    fn frame_sizes(&self, unit: &Unit, report: &[u8]) -> Vec<(FuncId, u64)> {
        let names = Emitter::names(unit, self.shard);
        let spelled: HashMap<String, FuncId> = unit
            .functions
            .iter()
            .map(|(id, _)| (names.spell(Emitter::func_ident(unit, id)).into_owned(), id))
            .collect();
        let mut frames: BTreeMap<FuncId, Frame> = BTreeMap::new();
        for line in String::from_utf8_lossy(report).lines() {
            let mut fields = line.split('\t');
            let (Some(place), Some(bytes)) = (fields.next(), fields.next()) else { continue };
            let Some(name) = place.rsplit(':').next() else { continue };
            let (name, clone) = name.split_once('.').map_or((name, None), |(n, c)| (n, Some(c)));
            let (Some(&id), Ok(bytes)) = (spelled.get(name), bytes.trim().parse::<u64>()) else { continue };
            let frame = frames.entry(id).or_default();
            if clone.is_some_and(|c| c.starts_with(PART)) {
                frame.parts += bytes;
            } else {
                frame.own = frame.own.max(bytes);
            }
        }
        frames.into_iter().map(|(id, f)| (id, f.own + f.parts)).collect()
    }
}

/// The suffix GCC gives the part of a function it splits out and calls.
const PART: &str = "part";

/// A function's frame as `.su` reports it: its own, or its largest clone's, and those of the
/// parts split out of it.
#[derive(Default)]
struct Frame {
    own: u64,
    parts: u64,
}

#[cfg(test)]
mod tests;
