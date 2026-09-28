//! QBE as a subprocess on IL text: small and simple, no vectors, no 32-bit targets.

use vun_backend::{BackendCaps, BackendInfo, Emits, Feature, Planned, Readiness};
use vun_lir_passes::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("qbe"),
    arches: Arches::Only(&[Name("x86_64"), Name("arm64"), Name("riscv64")]),
    caps: BackendCaps {
        emits: Emits::Assembly,
        features: &[Feature::CAbi],
        int_bits: 64,
        half: Half::Promoted,
    },
    readiness: Readiness::Planned,
};

/// Declared, generating no code yet.
pub const BACKEND: Planned = Planned(&INFO);
