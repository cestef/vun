use vun_lsp_protocol::base::TextDocumentPositionParams;
use vun_lsp_protocol::edit::{ReferenceContext, ReferenceParams, RenameParams};
use vun_lsp_protocol::requests as r;
use vun_lsp_protocol::rpc::ErrorCode;

use super::{at, served};

const SOURCE: &str = "\
Point := struct { x: u8 };
norm := (p: Point) -> u8 {
    n := p.x;
    n + n
};
twice := (p: Point) -> u8 { norm(p) + norm(p) };
";

fn references(at: TextDocumentPositionParams, include_declaration: bool) -> ReferenceParams {
    ReferenceParams { at, context: ReferenceContext { include_declaration } }
}

fn rename(at: TextDocumentPositionParams, new_name: &str) -> RenameParams {
    RenameParams { at, new_name: new_name.to_owned() }
}

/// The line and character where each range starts.
fn starts(ranges: impl Iterator<Item = vun_lsp_protocol::base::Range>) -> Vec<(u32, u32)> {
    ranges.map(|r| (r.start.line, r.start.character)).collect()
}

#[test]
fn references_of_a_local_are_its_binder_and_uses() {
    served("lsp-references-local", |client, uri| {
        client.open(uri, SOURCE);
        let found = client.request::<r::References>(&references(at(uri, 3, 4), true)).unwrap();
        assert_eq!(starts(found.iter().map(|l| l.range)), [(2, 4), (3, 4), (3, 8)]);
        let uses = client.request::<r::References>(&references(at(uri, 2, 4), false)).unwrap();
        assert_eq!(starts(uses.iter().map(|l| l.range)), [(3, 4), (3, 8)]);
    });
}

#[test]
fn references_of_a_declaration_are_found_by_what_names_resolve_to() {
    served("lsp-references-decl", |client, uri| {
        client.open(uri, SOURCE);
        // `norm` from one of its calls; the parameter `p` of `twice` is another name.
        let found = client.request::<r::References>(&references(at(uri, 5, 29), true)).unwrap();
        assert_eq!(starts(found.iter().map(|l| l.range)), [(1, 0), (5, 28), (5, 38)]);
    });
}

#[test]
fn renaming_edits_the_binder_and_every_use() {
    served("lsp-rename", |client, uri| {
        client.open(uri, SOURCE);
        let edit = client.request::<r::Rename>(&rename(at(uri, 0, 0), "Vec2")).unwrap().unwrap();
        assert_eq!(edit.document_changes.len(), 1);
        let changes = &edit.document_changes[0];
        assert_eq!(changes.text_document.version, Some(1), "an open document, at its version");
        assert!(changes.edits.iter().all(|e| e.new_text == "Vec2"));
        assert_eq!(starts(changes.edits.iter().map(|e| e.range)), [(0, 0), (1, 12), (5, 13)]);
        // A keyword is written escaped.
        let edit = client.request::<r::Rename>(&rename(at(uri, 2, 4), "match")).unwrap().unwrap();
        assert!(edit.document_changes[0].edits.iter().all(|e| e.new_text == "\\match"));
    });
}

#[test]
fn a_rename_to_what_is_not_a_name_fails() {
    served("lsp-rename-refused", |client, uri| {
        client.open(uri, SOURCE);
        let refused = client.request::<r::Rename>(&rename(at(uri, 0, 0), "not a name")).unwrap_err();
        assert_eq!(refused.code, ErrorCode::RequestFailed);
    });
}

#[test]
fn a_diagnostic_s_fix_is_a_code_action_over_its_range() {
    use vun_lsp_protocol::base::{Position, Range, TextDocumentIdentifier};
    use vun_lsp_protocol::edit::{CodeActionContext, CodeActionKind, CodeActionParams};

    served("lsp-code-actions", |client, uri| {
        client.open(uri, "x := (1;\n");
        let line =
            Range { start: Position { line: 0, character: 0 }, end: Position { line: 0, character: 8 } };
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: line,
            context: CodeActionContext { diagnostics: Vec::new(), only: None },
        };
        let actions = client.request::<r::CodeAction>(&params).unwrap();
        let closing = actions
            .iter()
            .flat_map(|a| a.edit.iter().flat_map(|e| &e.document_changes).flat_map(|c| &c.edits))
            .find(|e| e.new_text == ")");
        assert!(closing.is_some(), "the unclosed `(` is closed: {actions:?}");
        assert!(actions.iter().all(|a| a.kind == Some(CodeActionKind::QuickFix) && a.diagnostics.is_some()));
        // None for a range no diagnostic reaches.
        client.open(uri, "x := 1;\n");
        assert!(client.request::<r::CodeAction>(&params).unwrap().is_empty());
    });
}
