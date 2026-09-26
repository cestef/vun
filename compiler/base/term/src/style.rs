//! Semantic styles and the one palette mapping them to terminal attributes. Output names what a
//! piece of text is, never how it looks; the palette is cargo's (itself annotate-snippets'),
//! so help, status lines, tables, logs and diagnostics look like one tool.

use anstyle::{AnsiColor, Effects};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// Section titles and table heads: `Usage:`, `Commands:`.
    Heading,
    /// What the user writes as is: commands, options, names.
    Literal,
    /// What the user replaces: `<file>`.
    Placeholder,
    /// Progress and success: status verbs such as `Finished`.
    Good,
    Error,
    Warning,
    Note,
    Help,
    /// Positions and structure: line numbers, gutters.
    Accent,
    /// Where something happens: log targets and spans.
    Scope,
    /// The name of a value written `name=value`, such as a log field.
    Key,
    /// Content hashes and other identities, as jj shows change ids.
    Id,
    /// What matters less: planned commands, units, secondary columns.
    Dim,
    /// Plain text given weight.
    Strong,
}

impl Style {
    /// The terminal attributes of this style.
    #[must_use]
    pub const fn ansi(self) -> anstyle::Style {
        let bold = Effects::BOLD;
        match self {
            Self::Heading | Self::Good => AnsiColor::Green.on_default().effects(bold),
            Self::Literal => AnsiColor::Cyan.on_default().effects(bold),
            Self::Placeholder => AnsiColor::Cyan.on_default(),
            Self::Error => AnsiColor::BrightRed.on_default().effects(bold),
            Self::Warning => AnsiColor::BrightYellow.on_default().effects(bold),
            Self::Note => AnsiColor::BrightGreen.on_default().effects(bold),
            Self::Help => AnsiColor::BrightCyan.on_default().effects(bold),
            Self::Accent => AnsiColor::BrightBlue.on_default().effects(bold),
            Self::Id => AnsiColor::Magenta.on_default().effects(bold),
            Self::Scope => AnsiColor::Blue.on_default(),
            Self::Key => AnsiColor::Cyan.on_default().effects(Effects::ITALIC),
            Self::Dim => anstyle::Style::new().effects(Effects::DIMMED),
            Self::Strong => anstyle::Style::new().effects(bold),
        }
    }
}
