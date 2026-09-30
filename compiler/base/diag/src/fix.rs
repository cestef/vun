//! Labels and machine-applicable fixes.

use std::fmt;

use vun_codec::{Decode, Encode};
use vun_origin::OriginId;
use vun_span::Span;

use crate::Msg;

/// A location, optionally annotated. Renderers follow the origin chain to show generated code
/// together with its generator.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Label {
    pub origin: OriginId,
    pub msg: Option<Msg>,
}

/// How safe it is to apply a fix without review.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Hole {
    pub range: (u32, u32),
    pub choices: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Edit {
    pub span: Span,
    pub text: String,
    pub holes: Vec<Hole>,
}

impl Edit {
    /// `text` with `edits` applied, all of one file. Insertions at one offset keep their order.
    ///
    /// # Errors
    /// Returns why the edits do not apply: they overlap, span several files, or reach past
    /// `text` or inside a character.
    pub fn apply(edits: &[Self], text: &str) -> Result<String, Unapplied> {
        let mut sorted: Vec<&Self> = edits.iter().collect();
        sorted.sort_by_key(|e| (e.span.range.lo, e.span.range.hi));
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        for e in &sorted {
            if e.span.file != sorted[0].span.file {
                return Err(Unapplied::Files(sorted[0].span, e.span));
            }
            let lo = e.span.range.lo as usize;
            let unapplied = if lo < at { Unapplied::Overlap(e.span) } else { Unapplied::Outside(e.span) };
            let before = text.get(at..lo).ok_or(unapplied)?;
            text.get(lo..e.span.range.hi as usize).ok_or(Unapplied::Outside(e.span))?;
            out.push_str(before);
            out.push_str(&e.text);
            at = e.span.range.hi as usize;
        }
        out.push_str(&text[at..]);
        Ok(out)
    }
}

/// Why edits do not apply to a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unapplied {
    /// The edit starts before the one ahead of it ends.
    Overlap(Span),
    /// The edit reaches past the text, or inside a character.
    Outside(Span),
    /// The edits are of two files.
    Files(Span, Span),
}

impl fmt::Display for Unapplied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Overlap(_) => "an edit overlaps another",
            Self::Outside(_) => "an edit reaches outside the text",
            Self::Files(..) => "the edits are of several files",
        })
    }
}

impl std::error::Error for Unapplied {}

/// A proposed change; its edits never overlap.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
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

    /// A fix of one edit inserting `text` at the start of `at`.
    #[must_use]
    pub fn insert(msg: impl Into<Msg>, safety: Safety, at: Span, text: impl Into<String>) -> Self {
        Self::replace(msg, safety, Span::new(at.file, at.range.lo, at.range.lo), text)
    }

    /// A fix of one edit removing `span`.
    #[must_use]
    pub fn remove(msg: impl Into<Msg>, safety: Safety, span: Span) -> Self {
        Self::replace(msg, safety, span, String::new())
    }

    /// This fix, the one to apply among the diagnostic's.
    #[must_use]
    pub fn preferred(self) -> Self {
        Self { preferred: true, ..self }
    }
}
