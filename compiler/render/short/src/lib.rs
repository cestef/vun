//! One-line diagnostics: `file:line:col: severity[id]: message`.

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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use overt_diag::errors;
    use overt_span::{SourceFiles, Span};

    #[test]
    fn renders_location_and_relative_id() {
        let mut files = SourceFiles::new();
        let text = "norm1 := (p: Point) -> u32 {\n    p.x.abs() + p.y.abs()\n};\n";
        let f = files.add("src/geom.ov", text);
        let expr = "p.x.abs() + p.y.abs()";
        let lo = u32::try_from(text.find(expr).unwrap()).unwrap();
        let mut origins = OriginTable::new();
        let at = origins.source(Span::new(f, lo, lo + u32::try_from(expr.len()).unwrap()));
        let d =
            Diagnostic::error(errors::INT_DOES_NOT_FIT, "value does not fit the result type").at(at, None);

        let mut out = Vec::new();
        ShortRenderer::new(&mut out).emit(&d, &origins, &files);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "src/geom.ov:2:5: error[int.does_not_fit]: value does not fit the result type\n"
        );
    }
}
