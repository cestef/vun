//! Every Overt file of the repository lowers to HIR that verifies, and resolves.

use overt_diag::{DiagCtxt, DiagCx};
use overt_fs::Tree;
use overt_harness::repo_root;
use overt_hir::lower::Source;
use overt_hir::tree::Hir;
use overt_origin::OriginTable;
use overt_span::{EXTENSION, SourceFiles};
use overt_syntax::cst::Cst;
use overt_syntax::tokens::Tokens;

#[test]
fn repository_sources_lower_and_verify() {
    let root = repo_root();
    let mut failures = Vec::new();
    for dir in ["lib", "corpus/accept"] {
        for path in root.join(dir).files_with(EXTENSION).unwrap() {
            let text = std::fs::read_to_string(&path).unwrap();
            let file = SourceFiles::new().add(path.display().to_string(), text.as_str());
            let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
            let mut cx = DiagCx { diag: &mut diag, origins: &origins };
            let tokens = Tokens::lex(file, &text, &mut cx);
            let cst = Cst::parse(&tokens, &text, &mut cx);
            let hir = Hir::lower(Source { cst: &cst, tokens: &tokens, text: &text }, cx.origins);
            if let Err(malformed) = hir.verify(cx.origins) {
                failures.push(format!("{}: {malformed:?}", path.display()));
            }
            hir.resolve(&mut cx);
            if diag.error_count() > 0 {
                failures.push(format!(
                    "{}: {:?}",
                    path.display(),
                    diag.sorted().iter().map(|d| &d.id).collect::<Vec<_>>()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The origins of every declaration and expression of `text`, lowered in a fresh table.
fn origins(text: &str) -> overt_hash::Hash {
    let file = SourceFiles::new().add_as("pkg@1.0.0/pkg.ov", "pkg.ov", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    let hir = Hir::lower(Source { cst: &cst, tokens: &tokens, text }, cx.origins);
    let decls: Vec<_> = hir.decls.iter().map(|(_, d)| d.origin).collect();
    let exprs: Vec<_> = hir.exprs.iter().map(|(_, e)| e.origin).collect();
    overt_hash::Hash::of_value(&(decls, exprs))
}

#[test]
fn origins_survive_edits_before_their_declaration() {
    let decl = "limit := (a: u32) -> u32 { a + 1 };\n";
    assert_eq!(origins(decl), origins(&format!("// a comment added above\n\n{decl}")));
    assert_ne!(origins(decl), origins("limit := (a: u32) -> u32 { a  + 1 };\n"), "an edit inside is seen");
}
