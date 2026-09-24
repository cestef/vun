//! Terminal diagnostics through ariadne (spec 10, section 2): one color per label role, notes and
//! helps below the excerpt. ariadne has no patch view, so each fix is a help line naming its edits.

mod report;
#[cfg(test)]
mod tests;

use std::io::Write;

use vun_diag::{Color, DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_span::SourceMap;

pub struct AriadneRenderer<W: Write> {
    out: W,
    color: Color,
}

impl<W: Write> AriadneRenderer<W> {
    pub fn new(out: W, color: Color) -> Self {
        Self { out, color }
    }
}

impl<W: Write> DiagSink for AriadneRenderer<W> {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap) {
        // Best-effort output: a closed stream must not abort compilation.
        let _ = self.report(d, origins, sources);
    }
}
