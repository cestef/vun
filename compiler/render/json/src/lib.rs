//! Diagnostics as schema-versioned JSON lines (spec 10, section 2): one object per diagnostic,
//! `"type": "diagnostic"`, every location resolved to a file, lines and columns, every message
//! given as text and as its typed parts.

#[cfg(test)]
mod tests;
mod value;

use std::io::Write;

use vun_diag::{DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_span::SourceMap;

/// The version of the schema; it changes when a field changes meaning or goes away.
pub const SCHEMA: u64 = 1;

pub struct JsonRenderer<W: Write> {
    out: W,
}

impl<W: Write> JsonRenderer<W> {
    pub fn new(out: W) -> Self {
        Self { out }
    }
}

impl<W: Write> DiagSink for JsonRenderer<W> {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap) {
        let value = value::Cx { origins, sources }.diagnostic(d);
        // Best-effort output: a closed stream must not abort compilation.
        let _ = writeln!(self.out, "{value}");
    }
}
