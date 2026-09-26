//! Terminal output shared by every front end: output names what each piece of text is
//! ([`Style`]), one palette decides how it looks, and each stream decides whether it shows
//! colors ([`ColorChoice`], `--color`). Commands report [`Record`]s through a [`Shell`], which
//! shows them for people (results on standard output, status lines and diagnostics on standard
//! error) or as JSON lines on standard output ([`Format`], `--format`).

mod color;
mod markdown;
mod pager;
mod record;
mod shell;
mod style;
mod table;
#[cfg(test)]
mod tests;
mod text;
mod units;

pub use color::{Color, ColorChoice, Env, Stream};
pub use record::{Document, Format, Record, Row, Status};
pub use shell::{Out, Shell, Verb, Verbosity};
pub use style::Style;
pub use table::{Align, Column, Table};
pub use text::{Piece, Text};
pub use units::{Bytes, Counted, Elapsed, Noun};
