//! What library code reports with `lang.diag.error` (spec 10, section 3): its message, where it
//! points, what to do about it, and the edits that fix it.

use vun_codec::{Decode, Encode};
use vun_diag::{Diagnostic, Fix, Msg, Safety};
use vun_items::{Item, lang};
use vun_origin::{OriginId, OriginTable};
use vun_span::Span;

use crate::Value;

/// A `lang.diag.Report`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Report {
    pub message: String,
    pub at: Option<OriginId>,
    pub help: Option<String>,
    pub fixes: Vec<Edit>,
}

/// A `lang.diag.Fix`: the text replacing what the report points at.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Encode, Decode)]
pub struct Edit {
    pub msg: String,
    pub safety: Safety,
    pub text: String,
}

impl Report {
    /// A report of `message` alone, shown where the code that led to it is.
    #[must_use]
    pub fn message(message: &str) -> Self {
        Self { message: message.to_owned(), at: None, help: None, fixes: Vec::new() }
    }

    /// The report `value`, a `lang.diag.Report`, makes.
    #[must_use]
    pub fn of(value: &Value) -> Option<Self> {
        let some = |field: Item| match value.field(field)? {
            Value::Variant { payload, .. } => payload.first(),
            _ => None,
        };
        let at = match some(lang::diag_Report::at) {
            Some(Value::Origin(origin)) => Some(*origin),
            _ => None,
        };
        let help = some(lang::diag_Report::help).and_then(Value::text);
        let fixes = match value.field(lang::diag_Report::fixes) {
            Some(Value::Array(fixes)) => fixes.iter().filter_map(Edit::of).collect(),
            _ => Vec::new(),
        };
        Some(Self { message: value.field(lang::diag_Report::msg)?.text()?, at, help, fixes })
    }

    /// `d` with the help of this report, and its fixes when it points at `at`: each replaces
    /// what is written there.
    #[must_use]
    pub fn advise(&self, mut d: Diagnostic, at: Option<OriginId>) -> Diagnostic {
        if let Some(help) = &self.help {
            d = d.help(help.clone());
        }
        let Some(span) = at.map(|at| OriginTable::global().source_span(at)) else { return d };
        for edit in &self.fixes {
            d = d.fix(edit.fix(span));
        }
        d
    }
}

impl Edit {
    /// The edit a `lang.diag.Fix` makes.
    #[must_use]
    pub fn of(value: &Value) -> Option<Self> {
        Some(Self {
            msg: value.field(lang::diag_Fix::msg)?.text()?,
            safety: value.field(lang::diag_Fix::safety)?.safety()?,
            text: value.field(lang::diag_Fix::text)?.text()?,
        })
    }

    /// The fix replacing what is written at `span`.
    #[must_use]
    pub fn fix(&self, span: Span) -> Fix {
        Fix::replace(Msg::new().text(self.msg.clone()), self.safety, span, self.text.clone())
    }
}
