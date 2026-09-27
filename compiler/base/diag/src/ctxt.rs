//! Collecting diagnostics: the context, the sink interface and the error token.

use vun_codec::{Decode, Encode};
use vun_origin::{OriginTable, Resolve};
use vun_span::{SourceMap, Span};

use crate::{Diagnostic, Severity, TraceFrame};

/// Proof that an error was emitted. Only [`DiagCtxt`] creates one, so a poisoned type or value
/// carrying it guarantees that the build fails and that later diagnostics about it can be dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct ErrorGuaranteed(());

/// Output for diagnostics: terminal, short, JSON, SARIF, LSP.
pub trait DiagSink {
    /// Shows `d`, its positions where `origins` finds them.
    fn emit(&mut self, d: &Diagnostic, origins: &dyn Resolve, sources: &dyn SourceMap);
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Encode, Decode)]
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

    /// Records the diagnostics of `other` that are not recorded yet, each reached through
    /// `frame`: an error in a generic body, found in the instantiation a call reached.
    pub fn absorb_through(&mut self, other: &Self, frame: &TraceFrame) {
        for d in &other.diagnostics {
            let mut d = d.clone();
            d.trace.push(frame.clone());
            if !self.diagnostics.contains(&d) {
                self.diagnostics.push(d);
            }
        }
    }

    pub fn report(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    /// The guarantee that an error was reported, if one was.
    #[must_use]
    pub fn failed(&self) -> Option<ErrorGuaranteed> {
        (self.error_count() > 0).then_some(ErrorGuaranteed(()))
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.count(Severity::Error)
    }

    /// The diagnostics of `severity`.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics.iter().filter(|d| d.severity == severity).count()
    }

    /// All diagnostics in source order: by the file and place of their primary label's source,
    /// then id. The order does not depend on the order work reported them in, nor on where
    /// origins were recorded.
    #[must_use]
    pub fn sorted(&self, origins: &dyn Resolve) -> Vec<&Diagnostic> {
        let mut all: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        all.sort_by_cached_key(|d| {
            let at = d.primary.as_ref().map(|l| {
                let span = origins.source_span(l.origin);
                (span.file.key(), span.range.lo, span.range.hi)
            });
            (at, d.id.clone())
        });
        all
    }
}
