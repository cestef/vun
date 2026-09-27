//! The formatter on the repository's own sources, which it lays out already: it changes
//! whitespace only, and formatting twice changes nothing.

use std::path::{Path, PathBuf};

use vun_diag::{DiagCtxt, DiagCx};
use vun_fmt::Layout;
use vun_fs::Tree;
use vun_grammar::TokenKind;
use vun_harness::{REJECT, SOURCES, Snapshot, repo_root};
use vun_origin::OriginTable;
use vun_span::{EXTENSION, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

/// `text` formatted, or `None` when it does not parse cleanly.
fn formatted(text: &str) -> Option<String> {
    let file = SourceFiles::new().add("input.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let mut cx = DiagCx { diag: &mut diag, origins: &origins };
    let tokens = Tokens::lex(file, text, &mut cx);
    let cst = Cst::parse(&tokens, text, &mut cx);
    (diag.error_count() == 0).then(|| Layout::new(&tokens, &cst, text).to_string())
}

/// The tokens and comments of `text`, whitespace left out: what formatting keeps.
fn words(text: &str) -> Vec<(TokenKind, String)> {
    let file = SourceFiles::new().add("input.vun", text);
    let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
    let tokens = Tokens::lex(file, text, &mut DiagCx { diag: &mut diag, origins: &origins });
    tokens
        .lexemes()
        .filter(|l| l.kind != TokenKind::Whitespace)
        .map(|l| (l.kind, l.range.of(text).trim_end().to_owned()))
        .collect()
}

fn sources() -> Vec<PathBuf> {
    let root = repo_root();
    let dirs = SOURCES.iter().chain([&REJECT]);
    dirs.flat_map(|dir| root.join(dir).files_with(EXTENSION).expect("readable tree")).collect()
}

#[test]
fn repository_sources_are_laid_out_and_keep_their_tokens() {
    let mut failures = Vec::new();
    for path in sources() {
        let text = std::fs::read_to_string(&path).expect("readable source");
        let Some(once) = formatted(&text) else { continue };
        // The repository keeps one house style: `vun fmt` lays every source out.
        if once != text {
            failures.push(format!("{}: not laid out; run `vun fmt` on it", path.display()));
        }
        if words(&once) != words(&text) {
            failures.push(format!("{}: tokens changed", path.display()));
        }
        if formatted(&once).as_deref() != Some(once.as_str()) {
            failures.push(format!("{}: not stable", path.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn careless_inputs_format_as_snapshotted() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let changed =
        Snapshot::tree(&dir.join("inputs"), EXTENSION, &dir.join("snapshots"), "formatted", |text| {
            formatted(text).expect("inputs parse")
        })
        .expect("snapshot io");
    for c in &changed {
        eprintln!("{c}");
    }
    assert!(changed.is_empty(), "formatted inputs changed; rerun with VUN_BLESS=1 to accept");
}
