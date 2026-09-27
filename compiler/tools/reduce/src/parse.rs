//! A text as the reducer sees it: its tokens, its tree and how many errors parsing it reported.

use vun_diag::{DiagCtxt, DiagCx};
use vun_fmt::Layout;
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

pub(crate) struct Parse {
    pub(crate) tokens: Tokens,
    pub(crate) cst: Cst,
    pub(crate) errors: usize,
}

impl Parse {
    /// Parses `text` on its own, apart from the process's files.
    pub(crate) fn of(text: &str) -> Self {
        let file = SourceFiles::new().add("reduced.vun", text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        Self { tokens, cst, errors: diag.error_count() }
    }

    /// `text`, which parsed cleanly as this, laid out by the formatter.
    pub(crate) fn layout(&self, text: &str) -> String {
        Layout::new(&self.tokens, &self.cst, text).to_string()
    }
}
