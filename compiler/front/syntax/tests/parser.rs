//! Parsing of the repository's own sources: clean, verified, and snapshotted. The rejection
//! corpus runs through `cargo xtask corpus`.

use std::path::{Path, PathBuf};

use overt_diag::{DiagCtxt, DiagCx};
use overt_harness::{discover, repo_root, snapshot_tree};
use overt_origin::OriginTable;
use overt_span::SourceFiles;
use overt_syntax::cst::Cst;
use overt_syntax::lexer::lex;
use overt_syntax::parser::parse;
use overt_syntax::tokens::Tokens;

struct Parsed {
    tokens: Tokens,
    cst: Cst,
    diag: DiagCtxt,
}

fn parse_text(text: &str) -> Parsed {
    let file = SourceFiles::new().add("input.ov", text);
    let (mut diag, mut origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &mut origins };
    let tokens = lex(file, text, &mut cx);
    let cst = parse(&tokens, text, &mut cx);
    Parsed { tokens, cst, diag }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("readable source")
}

fn sources(dirs: &[&str]) -> Vec<PathBuf> {
    let root = repo_root();
    dirs.iter().flat_map(|dir| discover(&root.join(dir), "ov").expect("readable tree")).collect()
}

#[test]
fn repository_sources_parse_cleanly_and_verify() {
    let files = sources(&["corpus/accept", "lib"]);
    assert!(!files.is_empty());
    let mut failures = Vec::new();
    for path in files {
        let text = read(&path);
        let parsed = parse_text(&text);
        for d in parsed.diag.sorted() {
            failures.push(format!("{}: {}", path.display(), d.message));
        }
        if let Err(malformed) = parsed.cst.verify(&parsed.tokens) {
            failures.push(format!("{}: {malformed:?}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn corpus_cst_dumps_match_snapshots() {
    let snapshots = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/cst");
    let corpus = repo_root().join("corpus/accept");
    let changed = snapshot_tree(&corpus, "ov", &snapshots, "cst", |text| {
        let parsed = parse_text(text);
        parsed.cst.dump(&parsed.tokens, text).to_string()
    })
    .expect("snapshot io");
    for c in &changed {
        eprintln!("--- {}: expected\n{}\n--- actual\n{}", c.source.display(), c.expected, c.actual);
    }
    assert!(changed.is_empty(), "cst dumps changed; rerun with OVERT_BLESS=1 to accept");
}
