use vun_lsp_protocol::requests as r;

use super::{at, served};

const SOURCE: &str = "\
Point := struct { x: u8 };
/// The first coordinate.
norm := (p: Point) -> u8 { p.x };
add := (a: u8, b: u8) -> u8 { a + b };
sum := add(1, norm(Point{ .x = 2 }));
";

#[test]
fn a_call_being_written_shows_its_callee_s_signature_and_the_argument_filled() {
    served("lsp-signature", |client, uri| {
        client.open(uri, SOURCE);
        // Inside `add(1, `: the second parameter.
        let help = client.request::<r::SignatureHelp>(&at(uri, 4, 14)).unwrap().unwrap();
        let add = &help.signatures[0];
        assert_eq!(add.label, "add(a: u8, b: u8) -> u8");
        let params: Vec<_> = add.parameters.iter().flatten().map(|p| p.label.as_str()).collect();
        assert_eq!(params, ["a: u8", "b: u8"]);
        assert_eq!(help.active_parameter, Some(1));
        // Inside the inner call, its own callee, with its doc comment.
        let help = client.request::<r::SignatureHelp>(&at(uri, 4, 19)).unwrap().unwrap();
        assert_eq!(help.signatures[0].label, "norm(p: Point) -> u8");
        assert_eq!(help.active_parameter, Some(0));
        assert_eq!(
            help.signatures[0].documentation.as_ref().map(|d| d.value.as_str()),
            Some("The first coordinate.")
        );
        // Outside any call's arguments.
        assert_eq!(client.request::<r::SignatureHelp>(&at(uri, 0, 3)).unwrap(), None);
    });
}

#[test]
fn bindings_written_without_a_type_show_the_type_of_their_value() {
    use vun_lsp_protocol::assist::InlayHintParams;
    use vun_lsp_protocol::base::{Position, Range, TextDocumentIdentifier};

    served("lsp-inlay-hints", |client, uri| {
        client.open(uri, "f := (a: u8) -> u8 {\n    n := a;\n    n\n};\nk := f(1);\nG := struct {};\n");
        let whole = Range { start: Position::default(), end: Position { line: 6, character: 0 } };
        let params =
            InlayHintParams { text_document: TextDocumentIdentifier { uri: uri.clone() }, range: whole };
        let hints = client.request::<r::InlayHint>(&params).unwrap().unwrap_or_default();
        let at: Vec<_> = hints.iter().map(|h| (h.position.line, h.position.character)).collect();
        assert_eq!(at, [(1, 5), (4, 1)], "{hints:?}");
        assert!(hints.iter().all(|h| h.label.starts_with(": ")));
    });
}

#[test]
fn completions_are_members_after_a_dot_and_the_names_in_scope_elsewhere() {
    use vun_lsp_protocol::assist::CompletionItemKind;

    served("lsp-completion", |client, uri| {
        client.open(uri, "P := struct { x: u8, y: u8 };\nf := (p: P) -> u8 {\n    k := p.x;\n    k\n};\n");
        let labels = |items: &[vun_lsp_protocol::assist::CompletionItem]| {
            items.iter().map(|i| (i.label.clone(), i.kind)).collect::<Vec<_>>()
        };
        // After `p.`: the fields of `P`.
        let members = client.request::<r::Completion>(&at(uri, 2, 11)).unwrap();
        assert_eq!(
            labels(&members),
            [
                ("x".to_owned(), Some(CompletionItemKind::Field)),
                ("y".to_owned(), Some(CompletionItemKind::Field))
            ]
        );
        // In the body, after `k`'s binding: the parameter, the local and the file's declarations.
        let names: Vec<_> =
            client.request::<r::Completion>(&at(uri, 3, 4)).unwrap().into_iter().map(|i| i.label).collect();
        for name in ["P", "f", "k", "p"] {
            assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
        }
    });
}
