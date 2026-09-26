//! Diagnostic output formats, chosen with `--diagnostics` (spec 10, section 2). Each is a
//! renderer behind `DiagSink`; adding one is adding an entry to [`FORMATS`].

use std::io::Write;

use vun_command::{Args, Param, Values};
use vun_diag::{Color, DiagSink, Diagnostic};
use vun_render_annotate::AnnotateRenderer;
use vun_render_ariadne::AriadneRenderer;
use vun_render_short::ShortRenderer;

use crate::Session;

pub struct Format {
    pub name: &'static str,
    pub about: &'static str,
    pub sink: for<'w> fn(&'w mut dyn Write, Color) -> Box<dyn DiagSink + 'w>,
}

/// The first entry is the default.
pub const FORMATS: &[Format] = &[
    Format {
        name: "terminal",
        about: "source excerpts with labels, notes and fixes",
        sink: |out, color| Box::new(AnnotateRenderer::new(out, color)),
    },
    Format {
        name: "ariadne",
        about: "source excerpts with one color per label role",
        sink: |out, color| Box::new(AriadneRenderer::new(out, color)),
    },
    Format {
        name: "short",
        about: "one line per diagnostic: file:line:col: severity[id]: message",
        sink: |out, color| Box::new(ShortRenderer::new(out, color)),
    },
];

pub const DIAGNOSTICS: Param = Param::option("diagnostics")
    .about("how diagnostics are shown")
    .values(Values::Choice(|| FORMATS.iter().map(|f| f.name).collect()))
    .env("VUN_DIAGNOSTICS");

impl Format {
    /// The format used without `--diagnostics`: the first.
    pub const DEFAULT: &'static Self = &FORMATS[0];
}

/// Applies `--diagnostics` to the session; parsing checked the name.
///
/// # Errors
/// Never.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    if let Some(format) = args.choice(&DIAGNOSTICS, FORMATS, |f| f.name) {
        session.format = format;
    }
    Ok(())
}
