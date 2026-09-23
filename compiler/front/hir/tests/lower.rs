//! Every Overt file of the repository lowers to HIR that verifies, and resolves.

use overt_diag::{DiagCtxt, DiagCx};
use overt_harness::{discover, repo_root};
use overt_hir::lower::{Source, lower};
use overt_hir::{resolve, verify};
use overt_origin::OriginTable;
use overt_span::{EXTENSION, SourceFiles};
use overt_syntax::{lexer, parser};

#[test]
fn repository_sources_lower_and_verify() {
    let root = repo_root();
    let mut failures = Vec::new();
    for dir in ["lib", "corpus/accept"] {
        for path in discover(&root.join(dir), EXTENSION).unwrap() {
            let text = std::fs::read_to_string(&path).unwrap();
            let file = SourceFiles::new().add(path.display().to_string(), text.as_str());
            let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
            let mut cx = DiagCx { diag: &mut diag, origins: &origins };
            let tokens = lexer::lex(file, &text, &mut cx);
            let cst = parser::parse(&tokens, &text, &mut cx);
            let hir = lower(Source { cst: &cst, tokens: &tokens, text: &text }, cx.origins);
            if let Err(malformed) = verify::verify(&hir, cx.origins) {
                failures.push(format!("{}: {malformed:?}", path.display()));
            }
            resolve::resolve(&hir, &mut cx);
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
