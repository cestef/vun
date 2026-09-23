//! Diagnostic output formats, chosen with `--format` (spec 10, section 2). Each is a renderer
//! behind `DiagSink`; adding one is adding an entry to [`FORMATS`].

use std::io::Write;

use overt_command::{Args, Param, ParamKind};
use overt_diag::suggest::closest;
use overt_diag::{ChildKind, DiagSink, Diagnostic, Msg, errors};
use overt_render_annotate::{AnnotateRenderer, Color};
use overt_render_short::ShortRenderer;

use crate::Session;

pub struct Format {
    pub name: &'static str,
    pub about: &'static str,
    pub sink: fn(Box<dyn Write>, Color) -> Box<dyn DiagSink>,
}

/// The first entry is the default.
pub const FORMATS: &[Format] = &[
    Format {
        name: "terminal",
        about: "source excerpts with labels, notes and fixes",
        sink: |out, color| Box::new(AnnotateRenderer::new(out, color)),
    },
    Format {
        name: "short",
        about: "one line per diagnostic: file:line:col: severity[id]: message",
        sink: |out, _| Box::new(ShortRenderer::new(out)),
    },
];

pub const FORMAT: Param = Param {
    name: "format",
    short: None,
    about: "how diagnostics are shown: terminal (default) or short",
    kind: ParamKind::Option,
};

#[must_use]
pub fn default() -> &'static Format {
    &FORMATS[0]
}

/// Applies `--format` to the session.
///
/// # Errors
/// Returns a usage error naming the known formats when the value is not one of them.
pub fn configure(session: &mut Session, args: &Args) -> Result<(), Box<Diagnostic>> {
    let Some(name) = args.value(&FORMAT) else { return Ok(()) };
    session.format = FORMATS.iter().find(|f| f.name == name).ok_or_else(|| {
        let names = FORMATS.iter().map(|f| f.name);
        let msg = Msg::new().text("unknown format ").code(name);
        let help = match closest(name, names.clone()) {
            Some(near) => Msg::new().text("did you mean ").code(near).text("?"),
            None => Msg::new().text("formats: ").code_list(names),
        };
        Box::new(Diagnostic::error(errors::CLI_INVALID_VALUE, msg).child(ChildKind::Help, help))
    })?;
    Ok(())
}
