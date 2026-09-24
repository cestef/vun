//! The stages of the front end the driver runs so far, and the diagnostic groups each reports.
//!
//! Harnesses read this table to tell a missing diagnostic from one that belongs to a stage that
//! does not exist yet.

use overt_diag::{DiagId, errors};
use overt_span::FileId;

use crate::Session;

pub struct Stage {
    pub name: &'static str,
    pub reports: &'static [DiagId],
}

pub const STAGES: &[Stage] = &[
    Stage { name: "load", reports: &[errors::IO] },
    Stage { name: "syntax", reports: &[errors::SYNTAX] },
    Stage {
        name: "names",
        reports: &[
            errors::NAMES_DUPLICATE,
            errors::NAMES_SHADOWED,
            errors::NAMES_UNRESOLVED,
            errors::BINDING_IRREFUTABLE_TEST,
            errors::META_ESCAPE,
            errors::PACKAGE,
        ],
    },
    Stage {
        name: "types",
        reports: &[
            errors::TYPES_MISMATCH,
            errors::INT_WIDTH_CAP,
            errors::NAMES_PRIVATE,
            errors::NAMES_VISIBILITY,
            errors::COMPTIME,
            errors::IMPL,
        ],
    },
];

impl Stage {
    /// Whether some stage that exists reports `id`.
    #[must_use]
    pub fn implemented(id: &DiagId) -> bool {
        STAGES.iter().flat_map(|s| s.reports).any(|group| id.within(group))
    }
}

impl Session {
    /// Runs every stage that exists on `file`.
    pub fn check(&mut self, file: FileId) {
        self.parsed(file);
        self.check_names(file);
        self.check_types(file);
    }
}
