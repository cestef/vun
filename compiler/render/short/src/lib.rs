//! One-line diagnostics, `file:line:col: severity[id]: message`, each note and help on a line
//! of its own below.

use std::io::Write;

use vun_diag::{ChildKind, Color, DiagSink, Diagnostic};
use vun_origin::OriginTable;
use vun_span::SourceMap;
use vun_term::{Out, Style, Text};

pub struct ShortRenderer<W: Write> {
    out: W,
    color: Color,
}

impl<W: Write> ShortRenderer<W> {
    pub fn new(out: W, color: Color) -> Self {
        Self { out, color }
    }
}

/// Lines below the diagnostic.
trait ChildLine {
    /// `  = kind: msg`.
    fn line(self, msg: String) -> Text;
}

impl ChildLine for ChildKind {
    fn line(self, msg: String) -> Text {
        Text::of(Style::Accent, "  = ").styled(self.style(), self.name()).plain(": ").plain(msg)
    }
}

impl<W: Write> DiagSink for ShortRenderer<W> {
    fn emit(&mut self, d: &Diagnostic, origins: &OriginTable, sources: &dyn SourceMap) {
        let at = d.primary.as_ref().map(|label| {
            Text::new()
                .styled(Style::Strong, sources.locate(origins.source_span(label.origin)).to_string())
                .plain(": ")
        });
        let title = at
            .unwrap_or_default()
            .styled(d.severity.style(), format!("{}[{}]", d.severity, d.id))
            .styled(Style::Strong, ": ")
            .styled(Style::Strong, d.message.to_string());
        let fix = |f: &vun_diag::Fix| ChildKind::Help.line(format!("{} [{}]", f.msg, f.safety));
        let children = d
            .children
            .iter()
            .flat_map(|c| std::iter::once(c.kind.line(c.msg.to_string())).chain(c.fixes.iter().map(fix)));
        let trace = d.trace.iter().map(|frame| {
            let at = sources.locate(origins.source_span(frame.origin));
            ChildKind::Note.line(format!("{} at {at}", frame.msg))
        });
        let fixes = d.fixes.iter().map(fix);
        let mut out = Out::colored(&mut self.out, self.color);
        // Best-effort output: a closed stream must not abort compilation.
        for line in std::iter::once(title).chain(children).chain(trace).chain(fixes) {
            let _ = out.line(&line);
        }
    }
}

#[cfg(test)]
mod tests;
