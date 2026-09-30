use vun_driver::view::{Subject, View};
use vun_lsp_protocol::base::{Range, TextDocumentIdentifier};
use vun_lsp_protocol::command::{ExecuteCommandParams, ShowDocumentParams};
use vun_lsp_protocol::edit::{CodeActionContext, CodeActionKind, CodeActionParams};
use vun_lsp_protocol::{Request, requests as r};

use super::{at, served};
use crate::command::view::Viewed;

const SOURCE: &str = "P := struct { x: u8, y: u16 };\nk := 3;\nf := (p: P) -> u16 { p.y + k };\ng := () -> u16 { $(k * 2) };\n";

#[test]
fn each_view_of_a_file_is_a_source_action_whose_command_writes_and_shows_it() {
    served("lsp-views", |client, uri| {
        client.open(uri, SOURCE);
        let text_document = TextDocumentIdentifier { uri: uri.clone() };
        let context = CodeActionContext { diagnostics: Vec::new(), only: Some(vec![CodeActionKind::Source]) };
        let params = CodeActionParams { text_document, range: Range::default(), context };
        let actions = client.request::<r::CodeAction>(&params).unwrap();
        let commands: Vec<_> = actions.into_iter().filter_map(|a| a.command).collect();
        let viewed: Vec<Viewed> =
            commands.iter().map(|c| c.arguments.as_ref().unwrap()[0].read::<Viewed>().unwrap()).collect();
        // A file neither exporting a program nor a package's `build.vun` has the views of a file.
        let of_a_file: Vec<_> = View::ALL.iter().copied().filter(|v| v.subject() == Subject::File).collect();
        assert_eq!(viewed.iter().map(|v| v.view).collect::<Vec<_>>(), of_a_file);

        let layout = viewed.iter().position(|v| v.view == View::Layout).unwrap();
        let command = &commands[layout];
        let params =
            ExecuteCommandParams { command: command.command.clone(), arguments: command.arguments.clone() };
        assert_eq!(client.request::<r::ExecuteCommand>(&params).unwrap(), None);
        let path = View::Layout.beside(&uri.to_file_path().unwrap());
        assert!(!std::fs::read(&path).unwrap().is_empty());
        let asked = std::mem::take(&mut client.server.outbox);
        let shown = asked.iter().find(|m| m.method.as_deref() == Some(r::ShowDocument::METHOD)).unwrap();
        let shown: ShowDocumentParams = shown.params.as_ref().unwrap().read().unwrap();
        assert_eq!(shown.uri.to_file_path().unwrap(), std::path::absolute(&path).unwrap());
    });
}

#[test]
fn a_hover_shows_the_value_compiling_computes_and_the_layout_of_the_type() {
    served("lsp-rich-hover", |client, uri| {
        client.open(uri, SOURCE);
        // `k`, a value computed when compiling.
        let k = client.request::<r::Hover>(&at(uri, 2, 27)).unwrap().unwrap().contents.value;
        assert!(k.contains("k: ") && k.contains(" = 3"), "{k}");
        // Its own name, where it is declared, shows the same.
        let declared = client.request::<r::Hover>(&at(uri, 1, 0)).unwrap().unwrap().contents.value;
        assert_eq!(declared, k);
        // `p`, a value of `P`: its layout, and no value, which it has only when running.
        let p = client.request::<r::Hover>(&at(uri, 2, 21)).unwrap().unwrap().contents.value;
        assert!(p.contains("size 4, align 2") && !p.contains(" = "), "{p}");
        // `k * 2` under `$`, computed when compiling.
        let twice = client.request::<r::Hover>(&at(uri, 3, 21)).unwrap().unwrap().contents.value;
        assert!(twice.contains(" = 6"), "{twice}");
    });
}

#[test]
fn each_declaration_exporting_the_program_has_a_lens_running_it() {
    use vun_lsp_protocol::command::CodeLensParams;

    served("lsp-run-lens", |client, uri| {
        client.open(uri, "main := () -> () {};\nstart := std.os.start(main);\n");
        let params = CodeLensParams { text_document: TextDocumentIdentifier { uri: uri.clone() } };
        let lenses = client.request::<r::CodeLens>(&params).unwrap();
        let at: Vec<_> = lenses.iter().map(|l| (l.range.start.line, l.range.start.character)).collect();
        assert_eq!(at, [(1, 0)], "{lenses:?}");
        let command = lenses[0].command.as_ref().unwrap();
        assert_eq!(command.command, crate::command::Order::Run.name());
    });
}
