//! Terminal diagnostics through annotate-snippets (spec 10, section 2): source excerpts with
//! labels, notes and helps, fixes shown as patches, and the trace of generated code.

mod report;
#[cfg(test)]
mod tests;

use std::io::Write;

use annotate_snippets::Renderer;
use annotate_snippets::renderer::DecorStyle;
use vun_diag::{Color, DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_span::SourceMap;
use vun_term::Style;

pub struct AnnotateRenderer<W: Write> {
    out: W,
    renderer: Renderer,
}

impl<W: Write> AnnotateRenderer<W> {
    pub fn new(out: W, color: Color) -> Self {
        let renderer = match color {
            Color::Never => Renderer::plain(),
            Color::Always => Renderer::styled()
                .error(Style::Error.ansi())
                .warning(Style::Warning.ansi())
                .info(Style::Accent.ansi())
                .note(Style::Note.ansi())
                .help(Style::Help.ansi())
                .line_num(Style::Accent.ansi())
                .context(Style::Accent.ansi())
                .emphasis(Style::Strong.ansi()),
        };
        Self { out, renderer: renderer.decor_style(DecorStyle::Unicode) }
    }
}

impl<W: Write> DiagSink for AnnotateRenderer<W> {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap) {
        let report = report::Cx { origins, sources }.groups(d);
        // Best-effort output: a closed stream must not abort compilation.
        let _ = writeln!(self.out, "{}\n", self.renderer.render(&report));
    }
}
