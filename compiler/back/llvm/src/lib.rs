//! LLVM code generation: optimized builds and the widest native reach.

use vun_backend::{BackendCaps, BackendInfo, Emits, Feature, Planned, Readiness, Widest};
use vun_lir_passes::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("llvm"),
    arches: Arches::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[
            Feature::Vectors,
            Feature::TailCalls,
            Feature::NoAlias,
            Feature::FrameSizes,
            Feature::DebugInfo,
        ],
        widest: Widest::Bits(128),
        half: Half::Computed,
    },
    readiness: Readiness::Planned,
};

/// Declared, generating no code yet.
pub const BACKEND: Planned = Planned(&INFO);
