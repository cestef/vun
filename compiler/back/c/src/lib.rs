//! Strict C99 through the target's C compiler: reaches every target with one (spec 13, section
//! 8.2). The backend builds C as data (`ast`) from LIR (`emit`) and prints it once.

pub mod ast;
mod emit;
mod lack;

pub use lack::{Lack, Missing};

use std::collections::HashMap;
use vun_backend::{Artifact, ArtifactSink, Backend, BackendCaps, BackendInfo, Emits, Lines};
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
        c_abi: true,
        vectors: false,
        tail_calls: false,
        noalias: true,
        frame_sizes: true,
        debug_info: true,
    },
};

/// The functions a shard of a program holds at most (spec 13, section 2).
pub const SHARD: usize = 256;

pub struct CBackend {
    /// The functions a file holds at most; a larger program spans several.
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
            .map(|s| Artifact {
                name: s.name,
                kind: if s.header { Emits::Header } else { Emits::Source },
                bytes: s.file.to_string().into_bytes(),
            })
            .collect())
    }
}

impl Backend for CBackend {
    fn info(&self) -> &BackendInfo {
        &INFO
    }

    fn emit(
        &self,
        unit: &Unit,
        target: &TargetSpec,
        lines: &dyn Lines,
        out: &mut dyn ArtifactSink,
        diag: &mut DiagCtxt,
    ) -> Result<(), ErrorGuaranteed> {
        let artifacts =
            self.sources(unit, target, lines).map_err(|lack| diag.error(lack.diagnostic(INFO.id.0)))?;
        for artifact in artifacts {
            out.put(artifact);
        }
        Ok(())
    }

    /// From the `.su` files `-fstack-usage` writes: a line per function, `file:line:col:name`,
    /// its frame's bytes, and how they are known, separated by tabs.
    fn frame_sizes(&self, unit: &Unit, report: &[u8]) -> Vec<(FuncId, u64)> {
        let names = Emitter::names(unit, self.shard);
        let spelled: HashMap<String, FuncId> = unit
            .functions
            .iter()
            .map(|(id, _)| (names.spell(Emitter::func_ident(unit, id)).into_owned(), id))
            .collect();
        let text = String::from_utf8_lossy(report);
        let mut sizes: Vec<(FuncId, u64)> = text
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\t');
                let (place, bytes) = (fields.next()?, fields.next()?);
                let name = place.rsplit(':').next()?;
                Some((*spelled.get(name)?, bytes.trim().parse().ok()?))
            })
            .collect();
        sizes.sort_unstable();
        sizes
    }
}

#[cfg(test)]
mod tests;
