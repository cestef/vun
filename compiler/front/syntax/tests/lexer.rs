//! Lexing of the repository's own sources: clean, lossless, and snapshotted.

use std::path::{Path, PathBuf};

use overt_diag::{DiagCtxt, DiagCx};
use overt_harness::{Snapshot, discover, repo_root, snapshot};
use overt_origin::OriginTable;
use overt_span::SourceFiles;
use overt_syntax::lexer::lex;
use overt_syntax::tokens::Tokens;

fn lex_text(text: &str) -> (Tokens, DiagCtxt) {
    let file = SourceFiles::new().add("input.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = lex(file, text, &mut DiagCx { diag: &mut diag, origins: &mut origins });
    (tokens, diag)
}

fn sources() -> Vec<PathBuf> {
    let root = repo_root();
    ["corpus", "lib"].iter().flat_map(|dir| discover(&root.join(dir), "ov").expect("readable tree")).collect()
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
    for path in discover(&repo_root().join("corpus/accept"), "ov").expect("readable corpus") {
        let text = std::fs::read_to_string(&path).expect("readable source");
        let dump = lex_text(&text).0.dump(&text).to_string();
        let target = snapshots.join(path.file_name().expect("file name")).with_extension("tokens");
        if let Snapshot::Differs { expected, actual } = snapshot(&target, &dump).expect("snapshot io") {
            panic!(
                "token dump of {} changed; rerun with OVERT_BLESS=1 to accept\n--- expected\n{expected}\n--- actual\n{actual}",
                path.display()
            );
        }
    }
}
