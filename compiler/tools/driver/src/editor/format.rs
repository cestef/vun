//! A file laid out by the formatter, as `vun fmt` would write it.

use vun_diag::{DiagCtxt, DiagCx};
use vun_fmt::Layout;
use vun_span::FileId;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use crate::Session;

impl Session {
    /// `file` laid out; `None` when it does not parse, which is not formatted. What parsing
    /// reports is left to checking.
    #[must_use]
    pub fn formatted(&self, file: FileId) -> Option<String> {
        let text = self.text(file);
        let mut diag = DiagCtxt::new();
        let mut cx = DiagCx { diag: &mut diag, origins: self.origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        (diag.error_count() == 0).then(|| Layout::new(&tokens, &cst, text).to_string())
    }
}
