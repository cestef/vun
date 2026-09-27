use super::*;
use crate::tokens::Tokens;
use TokenKind::*;
use vun_diag::DiagCtxt;
use vun_diag::{DiagCx, DiagId, errors};
use vun_origin::OriginTable;
use vun_span::SourceFiles;

fn lex_text(text: &str) -> (Vec<(TokenKind, std::string::String)>, Vec<DiagId>) {
    let file = SourceFiles::new().add("t.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = Tokens::lex(file, text, &mut DiagCx { diag: &mut diag, origins: &origins });
    let significant = tokens.ids().map(|t| (tokens.kind(t), tokens.range(t).of(text).to_owned()));
    let ids = diag.sorted(&origins).into_iter().map(|d| d.id.clone()).collect();
    (significant.collect(), ids)
}

fn kinds(text: &str) -> Vec<TokenKind> {
    let (tokens, ids) = lex_text(text);
    assert!(ids.is_empty(), "unexpected diagnostics {ids:?} for {text:?}");
    tokens.into_iter().map(|(k, _)| k).collect()
}

#[test]
fn words() {
    assert_eq!(kinds("mut x := _y _ for"), [Mut, Ident, ColonEq, Ident, Underscore, For, Eof]);
}

#[test]
fn numbers() {
    assert_eq!(kinds("1_000 0xff 0o17 0b1010 1.5 1e-3 2E+8"), [Int, Int, Int, Int, Float, Float, Float, Eof]);
    assert_eq!(kinds("0..n"), [Int, DotDot, Ident, Eof]);
    assert_eq!(kinds("t.0.1"), [Ident, Dot, Int, Dot, Int, Eof]);
    assert_eq!(kinds("x.e1"), [Ident, Dot, Ident, Eof]);
}

#[test]
fn quotes() {
    assert_eq!(
        kinds(r#""a\n\x41\u{1F600}" 'x' '\'' 'é' 'outer:"#),
        [String, Char, Char, Char, Label, Colon, Eof]
    );
    assert_eq!(kinds("\\\\ raw line\nx"), [RawStringLine, Ident, Eof]);
    assert_eq!(kinds("\\\\ raw line\r\nx"), [RawStringLine, Ident, Eof]);
    assert_eq!(kinds(r#"\"C:\dir" \unsafe \x"#), [RawString, RawIdent, RawIdent, Eof]);
}

#[test]
fn literal_values() {
    assert_eq!(literal(String, r#""a\n\x41\u{e9}\"""#).as_deref(), Some("a\nA\u{e9}\""));
    assert_eq!(literal(RawString, r#"\"C:\dir""#).as_deref(), Some(r"C:\dir"));
    assert_eq!(literal(RawStringLine, r"\\ line").as_deref(), Some(" line"));
    assert_eq!(scalar(r"'\''"), Some('\''));
    assert_eq!(scalar(r"'\xff'"), Some('\u{ff}'));
    assert_eq!(scalar("'ab'"), None);
    assert_eq!(bytes(String, r#""\xff\u{e9}""#).as_deref(), Some(&[0xff, 0xc3, 0xa9][..]));
    assert_eq!(literal(String, r#""\xff""#), None);
    assert_eq!(literal(Ident, "x"), None);
}

#[test]
fn comments_are_trivia_and_docs_are_not() {
    assert_eq!(
        kinds("/// doc\n//! module\n// plain\n//// plain\nx"),
        [DocComment, ModuleDocComment, Ident, Eof]
    );
}

#[test]
fn punctuation_prefers_longest() {
    assert_eq!(
        kinds("x +%= 1; a..=b; p.*; #{ $x }"),
        [
            Ident,
            PlusPercentEq,
            Int,
            Semi,
            Ident,
            DotDotEq,
            Ident,
            Semi,
            Ident,
            DotStar,
            Semi,
            HashLBrace,
            Dollar,
            Ident,
            RBrace,
            Eof
        ]
    );
}

#[test]
fn errors_are_reported_once_and_lexing_continues() {
    let cases = [
        ("\"open", errors::SYNTAX_UNTERMINATED_STRING, vec![String, Eof]),
        ("'\\n", errors::SYNTAX_UNTERMINATED_CHAR, vec![Char, Eof]),
        (r#""\q" x"#, errors::SYNTAX_INVALID_ESCAPE, vec![String, Ident, Eof]),
        ("0b102 x", errors::SYNTAX_MALFORMED_NUMBER, vec![Int, Ident, Eof]),
        ("0x", errors::SYNTAX_MALFORMED_NUMBER, vec![Int, Eof]),
        ("@@ x", errors::SYNTAX_UNKNOWN_CHARACTER, vec![Error, Ident, Eof]),
        (r#"\"open"#, errors::SYNTAX_UNTERMINATED_STRING, vec![RawString, Eof]),
        ("\\ x", errors::SYNTAX_STRAY_ESCAPE, vec![Error, Ident, Eof]),
    ];
    for (text, id, expected) in cases {
        let (tokens, ids) = lex_text(text);
        assert_eq!(ids, [id], "{text:?}");
        assert_eq!(tokens.into_iter().map(|(k, _)| k).collect::<Vec<_>>(), expected, "{text:?}");
    }
}
