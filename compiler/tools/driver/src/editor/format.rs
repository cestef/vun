//! A file laid out by the formatter, as `vun fmt` would write it.

use vun_formatter::WIDTH;
use vun_span::FileId;

use crate::Session;

impl Session {
    /// The name of the formatter of Vun source.
    pub const FORMATTER: &str = "vun";

    /// `file` laid out; `None` when it does not parse, which is not formatted, or no formatter
    /// is there. What parsing reports is left to checking.
    #[must_use]
    pub fn formatted(&self, file: FileId) -> Option<String> {
        let formatter = self.registry.formatters.get(Self::FORMATTER)?;
        formatter.item.layout(self.text(file), WIDTH).ok()
    }
}
