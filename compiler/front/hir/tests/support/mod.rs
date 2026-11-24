use vun_diag::{DiagCtxt, DiagCx};
use vun_hir::lower::Source;
use vun_hir::tree::Hir;
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

/// `text` lowered to HIR, in a fresh origin table.
pub(crate) fn lowered(text: &str) -> Hir {
    let file = SourceFiles::new().add("pkg.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    Hir::lower(Source { cst: &cst, tokens: &tokens, text }, &mut cx)
}
