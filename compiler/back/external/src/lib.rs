//! External backends: any executable reading LIR text on standard input.

use vun_backend::{BackendCaps, BackendInfo, Emits, HeldFloats, Planned, Readiness, Widest};
use vun_lir_passes::legalize::Half;
use vun_target::{Name, Names};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("external"),
    arches: Names::Any,
    formats: Names::Any,
    caps: BackendCaps {
        emits: Emits::Object,
        features: &[],
        widest: Widest::Bits(64),
        half: Half::Promoted,
        floats: HeldFloats::Every,
    },
    readiness: Readiness::Planned,
    runs: &[],
};

/// Declared, generating no code yet.
pub const BACKEND: Planned = Planned(&INFO);
