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
        let path = client.server.session.shown_at(&uri.to_file_path().unwrap(), View::Layout.name());
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

#[test]
fn each_test_of_a_package_file_has_a_lens_running_it() {
    use vun_lsp_protocol::command::CodeLensParams;

    use vun_driver::testing::{self, Argument, Falsified, Outcome, Place, Ran, Said, Trapped};

    use crate::command::test::Tested;

    served("lsp-test-lens", |client, uri| {
        let dir = uri.to_file_path().unwrap().parent().unwrap().to_path_buf();
        let manifest = "package := .{ .name = \"a\", .id = 0x4402, .version = \"0.1.0\", .edition = 2026, \
                        .include = .{ \"build.vun\", \"a.vun\" } };\nbuild := (b: &mut std.build.Build) {};\n";
        std::fs::write(dir.join(vun_build::layout::MANIFEST), manifest).unwrap();
        client.open(uri, "#[test]\nadds := () { assert(1 + 1 == 2); };\n\nhelper := () {};\n");
        let text_document = TextDocumentIdentifier { uri: uri.clone() };
        let params = CodeLensParams { text_document: text_document.clone() };
        let lenses = client.request::<r::CodeLens>(&params).unwrap();
        let at: Vec<_> = lenses.iter().map(|l| (l.range.start.line, l.range.start.character)).collect();
        assert_eq!(at, [(1, 0)], "{lenses:?}");
        let command = lenses[0].command.as_ref().unwrap();
        assert_eq!(command.command, crate::command::Order::Test.name());
        let tested: Tested = command.arguments.as_ref().unwrap()[0].read().unwrap();
        assert_eq!(tested, Tested { text_document, name: "adds".to_owned() });

        // A run reports a trap: the lens says so, and the trap is shown where it fired, until
        // the document changes.
        let file = uri.to_file_path().unwrap().display().to_string();
        let said = Said {
            message: Some("assertion failed".to_owned()),
            at: Some(Place { file, line: 2, column: 21 }),
        };
        let trapped = Trapped { site: 0, said: Some(said) };
        let ran = Ran { name: "adds".to_owned(), outcome: Outcome::Trapped(trapped.clone()) };
        let tested = testing::Tested { ran, arguments: Vec::new(), output: Vec::new() };
        client.server.tested(uri.clone(), vec![tested]);
        let asked: Vec<_> = client.server.outbox.drain(..).filter_map(|m| m.method).collect();
        assert_eq!(asked, [r::CodeLensRefresh::METHOD]);
        let titles = |client: &mut super::Client<'_, '_>| -> Vec<String> {
            let lenses = client.request::<r::CodeLens>(&params).unwrap();
            lenses.into_iter().filter_map(|l| l.command).map(|c| c.title).collect()
        };
        assert_eq!(titles(client), ["Test: trapped"]);
        let published = client.published();
        let traps: Vec<_> =
            published[0].diagnostics.iter().map(|d| (d.range.start.line, d.range.start.character)).collect();
        assert_eq!(traps, [(1, 20)], "{published:?}");
        // A property's trap names the arguments of its failing case.
        let falsified = Falsified { trapped, seed: 7, case: 0, shrinks: 1 };
        let ran = Ran { name: "adds".to_owned(), outcome: Outcome::Falsified(falsified) };
        let arguments = vec![Argument { name: "x".to_owned(), value: 200_u64.into() }];
        client.server.tested(uri.clone(), vec![testing::Tested { ran, arguments, output: Vec::new() }]);
        client.server.outbox.clear();
        let messages: Vec<_> = client.published()[0].diagnostics.iter().map(|d| d.message.clone()).collect();
        assert_eq!(messages, ["the test `adds` was falsified by x = 200: assertion failed"]);
        assert_eq!(titles(client), ["Test: falsified"]);
        client.open(uri, "#[test]\nadds := () {};\n");
        assert_eq!(titles(client), ["Test"]);
        assert!(client.published()[0].diagnostics.is_empty());
    });
}
