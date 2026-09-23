//! Collecting diagnostics: the context, the sink interface and the error token.

use overt_origin::OriginTable;
use overt_span::{SourceMap, Span};

use crate::{Diagnostic, Severity};

/// Proof that an error was emitted. Only [`DiagCtxt`] creates one, so a poisoned type or value
/// carrying it guarantees that the build fails and that later diagnostics about it can be dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ErrorGuaranteed(());

/// Output for diagnostics: terminal, short, JSON, SARIF, LSP.
/// Whether a sink writes terminal colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Never,
    Always,
}

pub trait DiagSink {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap);
}

/// Where a stage reports: the diagnostics, and the origins their labels point to.
pub struct DiagCx<'a> {
    pub diag: &'a mut DiagCtxt,
    pub origins: &'a OriginTable,
}

impl DiagCx<'_> {
    /// Reports an error whose primary label is user-written source.
    pub fn error_at(&mut self, span: Span, d: Diagnostic) -> ErrorGuaranteed {
        let origin = self.origins.source(span);
        self.diag.error(d.at(origin, None))
    }
}

/// Collects diagnostics: for one compilation, or as the reports of one query, which are data.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct DiagCtxt {
    diagnostics: Vec<Diagnostic>,
}

impl DiagCtxt {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records an error and returns the proof that it was emitted.
    pub fn error(&mut self, mut d: Diagnostic) -> ErrorGuaranteed {
        d.severity = Severity::Error;
        self.diagnostics.push(d);
        ErrorGuaranteed(())
    }

    /// Records an error unless an equal one is recorded, and returns the proof that it was.
    pub fn error_once(&mut self, mut d: Diagnostic) -> ErrorGuaranteed {
        d.severity = Severity::Error;
        if !self.diagnostics.contains(&d) {
            self.diagnostics.push(d);
        }
        ErrorGuaranteed(())
    }

    /// Records the diagnostics of `other` not already recorded, since queries reached from
    /// several places return the same reports; the proof of an error when `other` holds one.
    pub fn absorb(&mut self, other: &Self) -> Option<ErrorGuaranteed> {
        for d in &other.diagnostics {
            if !self.diagnostics.contains(d) {
                self.diagnostics.push(d.clone());
            }
        }
        (other.error_count() > 0).then_some(ErrorGuaranteed(()))
    }

    pub fn report(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error).count()
    }

    /// All diagnostics in a deterministic order (by primary origin, then id), independent of the
    /// order in which parallel work reported them.
    #[must_use]
    pub fn sorted(&self) -> Vec<&Diagnostic> {
        let mut all: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        all.sort_by(|a, b| {
            let origin = |d: &Diagnostic| d.primary.as_ref().map(|l| l.origin);
            origin(a).cmp(&origin(b)).then_with(|| a.id.cmp(&b.id))
        });
        all
    }
}
