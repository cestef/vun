//! The formatter keeps every token of a file that parses, and formatting twice changes nothing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vun_diag::{DiagCtxt, DiagCx};
use vun_fmt::Layout;
use vun_grammar::TokenKind;
use vun_origin::OriginTable;
use vun_span::SourceFiles;
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

/// The tokens and comments of `text`, and its layout when it parses cleanly.
fn read(text: &str) -> (Vec<(TokenKind, String)>, Option<String>) {
    let file = SourceFiles::new().add("fuzz.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let words = tokens
        .lexemes()
        .filter(|l| l.kind != TokenKind::Whitespace)
        .map(|l| (l.kind, l.range.of(text).trim_end().to_owned()))
        .collect();
    let layout = (diag.error_count() == 0).then(|| Layout::new(&tokens, &cst, text).to_string());
    (words, layout)
}

fuzz_target!(|text: &str| {
    let (words, Some(once)) = read(text) else { return };
    let (kept, twice) = read(&once);
    assert_eq!(kept, words, "formatting changed a token");
    assert_eq!(twice.as_deref(), Some(once.as_str()), "formatting twice changed the file");
});
