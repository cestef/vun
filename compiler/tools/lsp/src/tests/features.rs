use vun_lsp_protocol::base::{TextDocumentIdentifier, TextDocumentParams};
use vun_lsp_protocol::language::{FormattingParams, SymbolKind, TokenModifier, TokenType};
use vun_lsp_protocol::requests as r;

use super::served;

const SOURCE: &str = "\
/// A point.
Point := struct {
    x: u8,
    y: u8,
};

Shape := enum { circle, square(u8) };

// Two lines
// of comment.
norm := (p: Point) -> u8 {
    mut n := p.x;
    n
};
";

fn whole(uri: &vun_uri::Uri) -> TextDocumentParams {
    TextDocumentParams { text_document: TextDocumentIdentifier { uri: uri.clone() } }
}

#[test]
fn the_outline_names_declarations_and_what_they_hold() {
    served("lsp-outline", |client, uri| {
        client.open(uri, SOURCE);
        let symbols = client.request::<r::DocumentSymbol>(&whole(uri)).unwrap();
        let top: Vec<_> = symbols.iter().map(|s| (s.name.as_str(), s.kind)).collect();
        assert_eq!(
            top,
            [("Point", SymbolKind::Struct), ("Shape", SymbolKind::Enum), ("norm", SymbolKind::Function)]
        );
        let fields: Vec<_> =
            symbols[0].children.iter().flatten().map(|s| (s.name.as_str(), s.kind)).collect();
        assert_eq!(fields, [("x", SymbolKind::Field), ("y", SymbolKind::Field)]);
        let variants: Vec<_> = symbols[1].children.iter().flatten().map(|s| s.name.as_str()).collect();
        assert_eq!(variants, ["circle", "square"]);
        assert_eq!((symbols[0].range.start.line, symbols[0].selection_range.start.line), (0, 1));
    });
}

#[test]
fn groups_and_comment_runs_fold() {
    served("lsp-folds", |client, uri| {
        client.open(uri, SOURCE);
        let folds = client.request::<r::FoldingRange>(&whole(uri)).unwrap();
        let lines: Vec<_> = folds.iter().map(|f| (f.start_line, f.end_line)).collect();
        assert_eq!(lines, [(1, 4), (8, 9), (10, 13)]);
    });
}

#[test]
fn formatting_is_one_edit_of_what_changes() {
    served("lsp-format", |client, uri| {
        client.open(uri, "x :=   1;\n");
        let params = FormattingParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            options: vun_data::Value::Null,
        };
        let edits = client.request::<r::Formatting>(&params).unwrap().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(
            (edits[0].range.start.character, edits[0].range.end.character, edits[0].new_text.as_str()),
            (5, 7, "")
        );
        client.open(uri, "x := (;\n");
        assert_eq!(client.request::<r::Formatting>(&params).unwrap(), None, "a file that does not parse");
    });
}

#[test]
fn semantic_tokens_class_names_by_what_they_declare() {
    served("lsp-tokens", |client, uri| {
        client.open(uri, SOURCE);
        let tokens = client.request::<r::SemanticTokens>(&whole(uri)).unwrap();
        let mut decoded = Vec::new();
        let (mut line, mut start) = (0, 0);
        for five in tokens.data.chunks(5) {
            line += five[0];
            start = if five[0] == 0 { start + five[1] } else { five[1] };
            decoded.push((line, start, five[2], TokenType::ALL[five[3] as usize], five[4]));
        }
        let find = |line, start| decoded.iter().find(|t| t.0 == line && t.1 == start).map(|t| (t.3, t.4));
        let declaration = TokenModifier::Declaration.bit();
        assert_eq!(find(0, 0), Some((TokenType::Comment, TokenModifier::Documentation.bit())));
        assert_eq!(find(1, 0), Some((TokenType::Type, declaration)));
        assert_eq!(find(10, 0), Some((TokenType::Function, declaration)));
        assert_eq!(find(10, 9), Some((TokenType::Parameter, declaration)));
        assert_eq!(find(10, 12), Some((TokenType::Type, 0)));
        assert_eq!(find(11, 4), Some((TokenType::Keyword, 0)));
        assert_eq!(find(11, 8), Some((TokenType::Variable, declaration | TokenModifier::Mutable.bit())));
        assert_eq!(find(11, 15), Some((TokenType::Property, 0)));
        assert_eq!(find(8, 0), Some((TokenType::Comment, 0)));
    });
}
