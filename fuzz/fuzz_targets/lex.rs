//! The lexer accepts any UTF-8 text and its output always verifies.

#![no_main]

use libfuzzer_sys::fuzz_target;
use overt_diag::{DiagCtxt, DiagCx};
use overt_origin::OriginTable;
use overt_span::SourceFiles;
use overt_syntax::lexer::lex;

fuzz_target!(|text: &str| {
    let file = SourceFiles::new().add("fuzz.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = lex(file, text, &mut DiagCx { diag: &mut diag, origins: &mut origins });
    if let Err(uncovered) = tokens.verify(text) {
        panic!("{uncovered:?}");
    }
});
