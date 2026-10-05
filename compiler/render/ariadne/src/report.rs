//! A diagnostic as an ariadne report over the files its labels point into.

use std::fmt;
use std::io::{self, Write};
use std::ops::Range;

use ariadne::{CharSet, Config, IndexType, Label, Report, ReportKind, sources};
use vun_diag::{ChildKind, Color, DiagSink, Diagnostic, Fix, Severity};
use vun_origin::{OriginId, Resolve};
use vun_render_short::ShortRenderer;
use vun_span::{FileId, SourceMap, Span};

use crate::AriadneRenderer;

/// A span as ariadne takes it: its file and a byte range.
type At = (Source, Range<usize>);

/// A file, as ariadne names it: by its path, told apart from other files of that path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Source {
    file: FileId,
    path: String,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}

/// Colors of labels by role.
const PRIMARY: ariadne::Color = ariadne::Color::Red;
const SECONDARY: ariadne::Color = ariadne::Color::Blue;
const CHILD: ariadne::Color = ariadne::Color::Cyan;

impl<W: Write> AriadneRenderer<W> {
    /// Writes `d` as an ariadne report.
    pub(crate) fn report(
        &mut self,
        d: &Diagnostic,
        origins: &dyn Resolve,
        sources_: &dyn SourceMap,
    ) -> io::Result<()> {
        let (color, out) = (self.color, &mut self.out);
        let at = |origin: OriginId| -> At {
            let span = origins.source_span(origin);
            let source = Source { file: span.file, path: sources_.file(span.file).shown() };
            (source, span.range.lo as usize..span.range.hi as usize)
        };
        let title = d.message.to_string();
        let Some(primary) = &d.primary else {
            // Without a location there is no excerpt to draw: the diagnostic is its lines alone.
            ShortRenderer::new(&mut *out, color).emit(d, origins, sources_);
            return Ok(());
        };
        let config = Config::default()
            .with_index_type(IndexType::Byte)
            .with_char_set(CharSet::Unicode)
            .with_color(color == Color::Always);
        let mut report = Report::build(kind(d.severity), at(primary.origin))
            .with_config(config)
            .with_code(&d.id)
            .with_message(title);
        let mut files: Vec<Span> = Vec::new();
        let mut label = |origin: OriginId, msg: Option<String>, color: ariadne::Color| {
            files.push(origins.source_span(origin));
            let label = Label::new(at(origin)).with_color(color);
            msg.map_or(label.clone(), |m| label.with_message(m))
        };
        // ariadne only draws the arrow of a label with a message; the primary one falls back to the
        // diagnostic's own.
        let primary_msg = primary.msg.as_ref().unwrap_or(&d.message).to_string();
        report.add_label(label(primary.origin, Some(primary_msg), PRIMARY));
        for l in &d.labels {
            report.add_label(label(l.origin, l.msg.as_ref().map(ToString::to_string), SECONDARY));
        }
        for child in &d.children {
            for l in &child.labels {
                let msg = l.msg.as_ref().unwrap_or(&child.msg).to_string();
                report.add_label(label(l.origin, Some(msg), CHILD));
            }
            let text = child.msg.to_string();
            match child.kind {
                ChildKind::Note => report.add_note(text),
                ChildKind::Help => report.add_help(text),
                ChildKind::Cause => report.add_note(format!("{}: {text}", ChildKind::Cause)),
            }
            for fix in &child.fixes {
                report.add_help(describe(fix, sources_));
            }
        }
        for frame in &d.trace {
            report.add_note(format!(
                "{} at {}",
                frame.msg,
                sources_.locate(origins.source_span(frame.origin))
            ));
        }
        for fix in &d.fixes {
            report.add_help(describe(fix, sources_));
        }
        let mut texts: Vec<(Source, String)> = Vec::new();
        for span in files {
            if !texts.iter().any(|(source, _)| source.file == span.file) {
                let file = sources_.file(span.file);
                texts.push((Source { file: span.file, path: file.shown() }, file.text.clone()));
            }
        }
        report.finish().write(sources(texts), out)
    }
}

/// A fix as one line: its message, its safety, then where each edit applies and, for a
/// replacement, what it replaces.
fn describe(fix: &Fix, sources: &dyn SourceMap) -> String {
    let edits: Vec<String> = fix
        .edits
        .iter()
        .map(|e| {
            let at = sources.locate(e.span);
            let old = e.span.range.of(&sources.file(e.span.file).text);
            if old.is_empty() { format!("at {at}") } else { format!("at {at}: `{old}` becomes `{}`", e.text) }
        })
        .collect();
    format!("{} [{}], {}", fix.msg, fix.safety, edits.join("; "))
}

fn kind(severity: Severity) -> ReportKind<'static> {
    match severity {
        Severity::Error => ReportKind::Error,
        Severity::Warning => ReportKind::Warning,
        Severity::Info | Severity::Hint => ReportKind::Advice,
    }
}
