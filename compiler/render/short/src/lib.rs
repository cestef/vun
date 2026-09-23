//! One-line diagnostics, `file:line:col: severity[id]: message`, each note and help on a line
//! of its own below.

use std::io::Write;

use overt_diag::{DiagSink, Diagnostic};
use overt_origin::OriginTable;
use overt_span::SourceMap;

pub struct ShortRenderer<W: Write> {
    out: W,
}

impl<W: Write> ShortRenderer<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }
}

impl<W: Write> DiagSink for ShortRenderer<W> {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap) {
        if let Some(label) = &d.primary {
            let span = origins.source_span(label.origin);
            let file = sources.file(span.file);
            let at = file.line_col(span.range.lo);
            let _ = write!(self.out, "{}:{}:{}: ", file.path, at.line, at.col);
        }
        // Best-effort output: a closed stream must not abort compilation.
        let _ = writeln!(self.out, "{}[{}]: {}", d.severity, d.id, d.message);
        for child in &d.children {
            let _ = writeln!(self.out, "  = {}: {}", child.kind, child.msg);
        }
    }
}

#[cfg(test)]
mod tests;
