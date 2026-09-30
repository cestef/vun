//! Lexing and parsing the repository's own sources, measured in bytes read.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use vun_diag::{DiagCtxt, DiagCx};
use vun_fs::Tree;
use vun_harness::{SOURCES, repo_root};
use vun_origin::OriginTable;
use vun_span::{EXTENSION, FileId, SourceFiles};
use vun_syntax::cst::Cst;
use vun_syntax::tokens::Tokens;

/// A source read once, and its tokens.
struct Source {
    file: FileId,
    text: String,
    tokens: Tokens,
}

impl Source {
    /// Every source of the repository, lexed.
    fn all() -> Vec<Self> {
        let files = SourceFiles::new();
        let root = repo_root();
        SOURCES
            .iter()
            .flat_map(|dir| root.join(dir).files_with(EXTENSION).expect("a readable tree"))
            .map(|path| {
                let text = std::fs::read_to_string(&path).expect("a readable source");
                let file = files.add(path.display().to_string(), text.as_str());
                let tokens = Self::lex(file, &text);
                Self { file, text, tokens }
            })
            .collect()
    }

    fn lex(file: FileId, text: &str) -> Tokens {
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        Tokens::lex(file, text, &mut DiagCx { diag: &mut diag, origins: &origins })
    }

    fn parse(&self) -> Cst {
        let (mut diag, origins) = (DiagCtxt::new(), OriginTable::new());
        Cst::parse(&self.tokens, &self.text, &mut DiagCx { diag: &mut diag, origins: &origins })
    }
}

fn syntax(c: &mut Criterion) {
    let sources = Source::all();
    let bytes = sources.iter().map(|s| s.text.len() as u64).sum();
    let mut group = c.benchmark_group("syntax");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("lex", |b| {
        b.iter(|| sources.iter().map(|s| Source::lex(s.file, &s.text)).collect::<Vec<_>>());
    });
    group.bench_function("parse", |b| b.iter(|| sources.iter().map(Source::parse).collect::<Vec<_>>()));
    group.finish();
}

criterion_group!(benches, syntax);
criterion_main!(benches);
