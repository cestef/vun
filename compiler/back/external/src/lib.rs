//! External backends: any executable reading LIR text on standard input.

use vun_backend::{BackendCaps, BackendInfo, Emits, Planned, Readiness};
use vun_lir::legalize::Half;
use vun_target::{Arches, Name};

pub const INFO: BackendInfo = BackendInfo {
    id: Name("external"),
    arches: Arches::Any,
    caps: BackendCaps { emits: Emits::Object, features: &[], int_bits: 64, half: Half::Promoted },
    readiness: Readiness::Planned,
};

/// Declared, generating no code yet.
pub const BACKEND: Planned = Planned(&INFO);
