//! In-process Cranelift code generation: the default for debug builds.

use vun_backend::{BackendCaps, BackendInfo, Emits, Feature, Planned, Readiness, Widest};
use vun_lir_passes::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("cranelift"),
    arches: Arches::Only(&[Name("x86_64"), Name("arm64"), Name("riscv64"), Name("s390x")]),
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[Feature::Vectors, Feature::TailCalls, Feature::DebugInfo],
        widest: Widest::Bits(128),
        half: Half::Promoted,
    },
    readiness: Readiness::Planned,
};

/// Declared, generating no code yet.
pub const BACKEND: Planned = Planned(&INFO);
