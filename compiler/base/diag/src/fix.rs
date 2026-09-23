//! Labels and machine-applicable fixes.

use std::fmt;

use overt_origin::OriginId;
use overt_span::Span;

use crate::Msg;

/// A location, optionally annotated. Renderers follow the origin chain to show generated code
/// together with its generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub origin: OriginId,
    pub msg: Option<Msg>,
}

/// How safe it is to apply a fix without review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Safety {
    /// Preserves meaning and compiles; the only kind applied automatically.
    Safe,
    /// May change behavior; applied only on request.
    Unsafe,
    /// Contains holes the user must fill.
    Placeholder,
    /// Shown, never applied.
    DisplayOnly,
}

impl fmt::Display for Safety {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Safe => "safe",
            Self::Unsafe => "unsafe",
            Self::Placeholder => "placeholder",
            Self::DisplayOnly => "display only",
        })
    }
}

/// A hole in a placeholder edit: a byte range of the edit text and the choices to offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hole {
    pub range: (u32, u32),
    pub choices: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub span: Span,
    pub text: String,
    pub holes: Vec<Hole>,
}

/// A proposed change; its edits never overlap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub msg: Msg,
    pub safety: Safety,
    pub edits: Vec<Edit>,
    pub preferred: bool,
}

impl Fix {
    /// A fix of one edit replacing `span` with `text`; an empty span inserts.
    #[must_use]
    pub fn replace(msg: impl Into<Msg>, safety: Safety, span: Span, text: impl Into<String>) -> Self {
        let edit = Edit { span, text: text.into(), holes: Vec::new() };
        Self { msg: msg.into(), safety, edits: vec![edit], preferred: false }
    }
}
