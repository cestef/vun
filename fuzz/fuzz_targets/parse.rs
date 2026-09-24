//! The parser accepts any token stream and its tree always verifies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use overt_diag::{DiagCtxt, DiagCx};
use overt_origin::OriginTable;
use overt_span::SourceFiles;
use overt_syntax::cst::Cst;
use overt_syntax::tokens::Tokens;

fuzz_target!(|text: &str| {
    let file = SourceFiles::new().add("fuzz.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    if let Err(malformed) = cst.verify(&tokens) {
        panic!("{malformed:?}");
    }
});
