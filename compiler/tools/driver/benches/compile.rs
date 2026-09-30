//! The compiler end to end over the corpus's programs, each copied into a scratch directory:
//! checking and generating code from nothing (every library computed), checking from the memos
//! an earlier run saved, and checking again after an edit in the same process.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use vun_driver::codegen::Listed;
use vun_driver::{Persistence, Session};
use vun_fs::Scratch;
use vun_harness::{ACCEPT, repo_root};
use vun_query::Db;
use vun_span::{EXTENSION, FileId};

/// The programs measured: the smallest, the tour of the README, and one spawning threads.
const PROGRAMS: [&str; 3] = ["hello", "tour", "threads"];

/// A program of the corpus in a directory of its own, where its saved queries go.
struct Program {
    name: &'static str,
    scratch: Scratch,
    path: PathBuf,
    text: String,
}

impl Program {
    fn all() -> Vec<Self> {
        PROGRAMS.into_iter().map(Self::new).collect()
    }

    fn new(name: &'static str) -> Self {
        let file = PathBuf::from(name).with_extension(EXTENSION);
        let text = std::fs::read_to_string(repo_root().join(ACCEPT).join("programs").join(&file))
            .expect("a program of the corpus");
        let scratch = Scratch::new(name);
        let path = scratch.write(&file, &text);
        Self { name, scratch, path, text }
    }

    /// A session over `db` with the program loaded as `text`.
    fn open(&self, db: &Arc<Db>, text: String) -> (Session, FileId) {
        let mut session = Session::sharing(db);
        let file = session.open(&self.path, text);
        (session, file)
    }

    /// A session saving its queries beside the program, and the libraries' memos in the scratch
    /// directory, as `vun` does in the store.
    fn persisted(&self) -> (Session, FileId) {
        let mut session = Session::new();
        session.persistence = Persistence::On;
        session.shared = Some(self.scratch.join("shared"));
        let file = session.load(&self.path).expect("a readable program");
        (session, file)
    }

    /// The program's text with a declaration of its own appended, whose value is `n`.
    fn edited(&self, n: u64) -> String {
        format!("{}\nedited := () -> u64 {{ {n} }};\n", self.text)
    }
}

/// Checks `file` and requires that it passes.
fn checked((mut session, file): (Session, FileId)) -> (Session, FileId) {
    session.check(file);
    assert!(!session.failed(), "the program checks");
    (session, file)
}

/// Checks `file`, generates its code and requires that both pass.
fn coded((session, file): (Session, FileId)) -> Session {
    let (mut session, file) = checked((session, file));
    assert!(session.code(file, Listed::No).is_ok(), "the program generates code");
    session
}

/// Every library computed, in a process that has computed nothing.
fn cold(c: &mut Criterion, programs: &[Program]) {
    let mut group = c.benchmark_group("cold");
    group.sample_size(10);
    for program in programs {
        let fresh = || program.open(&Arc::new(Db::new()), program.text.clone());
        group.bench_function(BenchmarkId::new("check", program.name), |b| {
            b.iter_with_large_drop(|| checked(fresh()));
        });
        group.bench_function(BenchmarkId::new("code", program.name), |b| {
            b.iter_with_large_drop(|| coded(fresh()));
        });
    }
    group.finish();
}

/// A new process checking a program an earlier one checked and saved.
fn saved(c: &mut Criterion, programs: &[Program]) {
    let mut group = c.benchmark_group("saved");
    for program in programs {
        let (session, _) = checked(program.persisted());
        session.save();
        drop(session);
        group.bench_function(BenchmarkId::new("check", program.name), |b| {
            b.iter_with_large_drop(|| checked(program.persisted()));
        });
    }
    group.finish();
}

/// A declaration appended to the program, then checked again over the same queries, as the
/// language server does after each change.
fn edit(c: &mut Criterion, programs: &[Program]) {
    let mut group = c.benchmark_group("edit");
    for program in programs {
        let db = Arc::new(Db::new());
        drop(checked(program.open(&db, program.text.clone())));
        let mut n = 0;
        group.bench_function(BenchmarkId::new("check", program.name), |b| {
            b.iter_with_large_drop(|| {
                n += 1;
                checked(program.open(&db, program.edited(n)))
            });
        });
    }
    group.finish();
}

fn compile(c: &mut Criterion) {
    let programs = Program::all();
    cold(c, &programs);
    saved(c, &programs);
    edit(c, &programs);
}

criterion_group! {
    name = benches;
    config = Criterion::default().warm_up_time(Duration::from_secs(1));
    targets = compile
}
criterion_main!(benches);
