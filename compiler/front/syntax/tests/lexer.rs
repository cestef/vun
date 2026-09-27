//! Lexing of the repository's own sources: clean, lossless, and snapshotted.

use std::path::{Path, PathBuf};
use vun_fs::Tree;

use vun_diag::{DiagCtxt, DiagCx};
use vun_harness::{ACCEPT, SOURCES, Snapshot, repo_root};
use vun_origin::OriginTable;
use vun_span::{EXTENSION, SourceFiles};
use vun_syntax::tokens::Tokens;

fn lex_text(text: &str) -> (Tokens, DiagCtxt) {
    let file = SourceFiles::new().add("input.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = Tokens::lex(file, text, &mut DiagCx { diag: &mut diag, origins: &origins });
    (tokens, diag)
}

fn sources() -> Vec<PathBuf> {
    let root = repo_root();
    SOURCES.iter().flat_map(|dir| root.join(dir).files_with(EXTENSION).expect("readable tree")).collect()
}

#[test]
fn repository_sources_lex_cleanly_and_losslessly() {
    let files = sources();
    assert!(!files.is_empty());
    for path in files {
        let text = std::fs::read_to_string(&path).expect("readable source");
        let (tokens, diag) = lex_text(&text);
        assert_eq!(diag.error_count(), 0, "lexer errors in {}", path.display());
        assert_eq!(tokens.verify(&text), Ok(()), "{}", path.display());
    }
}

#[test]
fn corpus_token_dumps_match_snapshots() {
    let snapshots = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/tokens");
    let corpus = repo_root().join(ACCEPT);
    let changed = Snapshot::tree(&corpus, EXTENSION, &snapshots, "tokens", |text| {
        lex_text(text).0.dump(text).to_string()
    })
    .expect("snapshot io");
    for c in &changed {
        eprintln!("{c}");
    }
    assert!(changed.is_empty(), "tokens dumps changed; rerun with VUN_BLESS=1 to accept");
}
