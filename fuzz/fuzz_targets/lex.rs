//! The lexer accepts any UTF-8 text and its output always verifies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_diag::{DiagCtxt, DiagCx};
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::tokens::Tokens;

fuzz_target!(|text: &str| {
    let file = SourceFiles::new().add("fuzz.vun", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = Tokens::lex(file, text, &mut DiagCx { diag: &mut diag, origins: &mut origins });
    if let Err(uncovered) = tokens.verify(text) {
        panic!("{uncovered:?}");
    }
});
