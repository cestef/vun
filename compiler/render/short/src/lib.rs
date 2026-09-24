//! One-line diagnostics, `file:line:col: severity[id]: message`, each note and help on a line
//! of its own below.

use std::io::Write;

use vun_diag::{ChildKind, DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_span::SourceMap;

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
            let _ = write!(self.out, "{}: ", sources.locate(origins.source_span(label.origin)));
        }
        // Best-effort output: a closed stream must not abort compilation.
        let _ = writeln!(self.out, "{}[{}]: {}", d.severity, d.id, d.message);
        for child in &d.children {
            let _ = writeln!(self.out, "  = {}: {}", child.kind, child.msg);
        }
        for fix in &d.fixes {
            let _ = writeln!(self.out, "  = {}: {} [{}]", ChildKind::Help, fix.msg, fix.safety);
        }
    }
}

#[cfg(test)]
mod tests;
