//! Any text lowers to HIR that verifies, and resolves without panicking.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_diag::{DiagCtxt, DiagCx};
use vun_hir::lower::Source;
use vun_hir::tree::Hir;
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

fuzz_target!(|text: &str| {
    let file = SourceFiles::new().add("fuzz.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let hir = Hir::lower(Source { cst: &cst, tokens: &tokens, text }, &origins);
    if let Err(malformed) = hir.verify(&origins) {
        panic!("{malformed:?}");
    }
    hir.resolve(&mut cx);
});
