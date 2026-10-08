//! Every Vun file of the repository lowers to HIR that verifies, and resolves.

use vun_diag::{DiagCtxt, DiagCx};
use vun_fs::Tree;
use vun_harness::{SOURCES, repo_root};
use vun_hir::lower::Source;
use vun_hir::tree::Hir;
use vun_origin::OriginTable;
use vun_span::{EXTENSION, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

mod common;

#[test]
fn repository_sources_lower_and_verify() {
    let root = repo_root();
    let mut failures = Vec::new();
    for dir in SOURCES {
        for path in root.join(dir).files_with(EXTENSION).unwrap() {
            let text = std::fs::read_to_string(&path).unwrap();
            let file = SourceFiles::new().add(path.display().to_string(), text.as_str());
            let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
            let mut cx = DiagCx { diag: &mut diag, origins: &origins };
            let tokens = Tokens::lex(file, &text, &mut cx);
            let cst = Cst::parse(&tokens, &text, &mut cx);
            let hir = Hir::lower(Source { cst: &cst, tokens: &tokens, text: &text }, &mut cx);
            if let Err(malformed) = hir.verify(cx.origins) {
                failures.push(format!("{}: {malformed:?}", path.display()));
            }
            hir.resolve(&mut cx);
            if diag.error_count() > 0 {
                failures.push(format!(
                    "{}: {:?}",
                    path.display(),
                    diag.sorted(&origins).iter().map(|d| &d.id).collect::<Vec<_>>()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The origins of every declaration and expression of `text`, lowered in a fresh table.
fn origins(text: &str) -> vun_hash::Hash {
    let hir = common::lowered(text);
    let decls: Vec<_> = hir.decls.iter().map(|(_, d)| d.origin).collect();
    let exprs: Vec<_> = hir.exprs.iter().map(|(_, e)| e.origin).collect();
    vun_hash::Hash::of_value(&(decls, exprs))
}

#[test]
fn origins_survive_edits_before_their_declaration() {
    let decl = "limit := (a: u32) -> u32 { a + 1 };\n";
    assert_eq!(origins(decl), origins(&format!("// a comment added above\n\n{decl}")));
    assert_ne!(origins(decl), origins("limit := (a: u32) -> u32 { a  + 1 };\n"), "an edit inside is seen");
}

#[test]
fn a_tuple_position_past_u32_is_reported() {
    let ids = |text: &str| {
        let file = SourceFiles::new().add("pkg.vun", text);
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        let mut cx = DiagCx { diag: &mut diag, origins: &origins };
        let tokens = Tokens::lex(file, text, &mut cx);
        let cst = Cst::parse(&tokens, text, &mut cx);
        let _ = Hir::lower(Source { cst: &cst, tokens: &tokens, text }, &mut cx);
        diag.sorted(&origins).iter().map(|d| d.id).collect::<Vec<_>>()
    };
    assert_eq!(ids("x := t.4294967295;\n"), []);
    assert_eq!(ids("x := t.4294967296;\n"), [vun_diag::errors::SYNTAX_TUPLE_INDEX_TOO_LARGE]);
}
