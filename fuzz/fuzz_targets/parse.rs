//! The parser accepts any token stream and its tree always verifies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

fuzz_target!(|text: &str| {
    let file = SourceFiles::new().add("fuzz.vun", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    if let Err(malformed) = cst.verify(&tokens) {
        panic!("{malformed:?}");
    }
});
