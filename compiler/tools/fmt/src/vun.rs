//! The formatter of Vun source as a component's service.

use vun_diag::{DiagCtxt, DiagCx};
use vun_formatter::{Formatter, Unlaid};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

use crate::Layout;

/// The formatter of Vun source: whitespace from the CST and the grammar's tables.
pub struct Vun;

impl Formatter for Vun {
    fn name(&self) -> &'static str {
        "vun"
    }

    /// Parses `text` on its own, apart from the process's files.
    fn layout(&self, text: &str, width: usize) -> Result<String, Unlaid> {
        let file = SourceFiles::new().add("formatted.vun", text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        if diag.error_count() > 0 {
            return Err(Unlaid::Unparsed);
        }
        Ok(Layout::new(&tokens, &cst, text).width(width).to_string())
    }
}
