//! `cargo xtask tree-sitter`: the tree-sitter parser of `editors/tree-sitter`, generated from the
//! `grammar.js` that `cargo gen` writes, then every source of the repository parsed with it and
//! its tree compared with the compiler's (see [`compare`]). Each file whose trees differ is a
//! finding, with its first differences.

mod compare;
mod conflicts;

use std::io;
use std::path::{Path, PathBuf};

use vun_command::{Args, Outcome, Shell};
use vun_diag::{ChildKind, DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::Tree as _;
use vun_harness::{SOURCES, repo_root};
use vun_par::Parallel;
use vun_span::EXTENSION;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Failure, Invocation, Tool};
use vun_tree_sitter::{Node, Tree};

use crate::task::Program;
use compare::{Difference, Spanned};

/// The grammar's directory, relative to the repository root.
const GRAMMAR: &str = "editors/tree-sitter";
/// The CLI's commands: generating the parser, and printing the trees of files.
const GENERATE: &str = "generate";
const PARSE: &str = "parse";
/// Files parsed by one run of the CLI.
const BATCH: usize = 64;
/// Differences shown per file.
const SHOWN: usize = 3;

const FAILED: DiagId = DiagId::new("xtask.tree_sitter.failed");
const DIFFERS: DiagId = DiagId::new("xtask.tree_sitter.differs");

pub(crate) fn check(diag: &mut DiagCtxt, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let cli = match Program::TreeSitter.find() {
        Ok(cli) => cli,
        Err(d) => {
            diag.error(*d);
            return Ok(Outcome::Failure);
        }
    };
    let root = repo_root();
    let dir = root.join(GRAMMAR);
    if let Err(d) = conflicts::settle(&cli, &dir) {
        diag.error(*d);
        return Ok(Outcome::Failure);
    }
    let mut files = Vec::new();
    for source in SOURCES {
        files.extend(root.join(source).files_with(EXTENSION)?);
    }
    let batches: Vec<&[PathBuf]> = files.chunks(BATCH).collect();
    // One run at a time: the first builds the parser into the CLI's cache, which runs at once race on.
    let compared = batches.iter().map(|batch| compare(&cli, &dir, batch));
    let (mut agreeing, mut outcome) = (0, Outcome::Success);
    for (batch, result) in batches.iter().zip(compared) {
        let relative = |path: &Path| path.strip_prefix(&root).unwrap_or(path).display().to_string();
        match result {
            Ok(found) => {
                for (path, found) in batch.iter().zip(found) {
                    if found.is_empty() {
                        agreeing += 1;
                    } else {
                        diag.error(finding(&relative(path), &found));
                        outcome = Outcome::Failure;
                    }
                }
            }
            Err(d) => {
                diag.error(*d);
                outcome = Outcome::Failure;
            }
        }
    }
    shell.status(&Compared { agreeing, total: files.len() });
    Ok(outcome)
}

fn run(cli: &Executable, dir: &Path, args: Vec<Arg>) -> Result<vun_tool::Output, Failure> {
    let inv = Invocation { args, cwd: Some(dir.to_owned()), ..Invocation::default() };
    cli.run(&inv).map_err(Failure::Start)
}

/// The differences of each file of `batch`, parsed by one run of the CLI, which fails on a file
/// with errors and still prints its tree.
fn compare(cli: &Executable, dir: &Path, batch: &[PathBuf]) -> Result<Vec<Vec<Difference>>, Box<Diagnostic>> {
    let unread = |what: &'static str| {
        let msg =
            Msg::new().text("could not read ").text(what).text(" of ").code(batch[0].display().to_string());
        Box::new(Diagnostic::error(FAILED, msg))
    };
    let mut args = vec![Arg::from(PARSE)];
    args.extend(batch.iter().cloned().map(Arg::Path));
    let out = run(cli, dir, args).map_err(|f| unread("the trees").caused_by(&f))?;
    let printed = String::from_utf8_lossy(&out.stdout);
    let trees = Tree::read(&printed).map_err(|e| unread("the trees").caused_by(&e))?;
    if trees.roots.len() != batch.len() {
        let msg = Msg::new()
            .text("`tree-sitter parse` printed ")
            .num(u32::try_from(trees.roots.len()).unwrap_or(u32::MAX))
            .text(" trees for ")
            .num(u32::try_from(batch.len()).unwrap_or(u32::MAX))
            .text(" files from ")
            .code(batch[0].display().to_string());
        return Err(Box::new(Diagnostic::error(FAILED, msg)));
    }
    let pairs: Vec<(&PathBuf, &Node)> = batch.iter().zip(&trees.roots).collect();
    pairs
        .par_map(|&(path, root)| {
            let text = std::fs::read_to_string(path).map_err(|e| unread("the text").caused_by(&e))?;
            Ok(compare::differences(&text, root))
        })
        .into_iter()
        .collect()
}

/// A file whose trees differ, with its first differences.
fn finding(path: &str, found: &[Difference]) -> Diagnostic {
    let msg = Msg::new().code(path).text(": its tree-sitter tree differs from the compiler's");
    let shown = found.iter().take(SHOWN).map(|d| match d {
        Difference::Error((start, end)) => {
            Msg::new().text("an error at bytes ").num(*start).text("..").num(*end)
        }
        Difference::Missing(n) => node(Msg::new().text("no "), n),
        Difference::Extra(n) => node(Msg::new().text("an extra "), n),
    });
    shown.fold(Diagnostic::error(DIFFERS, msg), |d, m| d.child(ChildKind::Note, m))
}

fn node(msg: Msg, n: &Spanned) -> Msg {
    msg.code(n.kind).text(" at bytes ").num(n.bytes.0).text("..").num(n.bytes.1)
}

/// How many files the two parsers agree on.
struct Compared {
    agreeing: usize,
    total: usize,
}

impl Record for Compared {
    fn kind(&self) -> &'static str {
        "tree-sitter"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, format!("{} of {} files", self.agreeing, self.total))
    }

    fn fields(&self) -> vun_data::Object<'static> {
        vun_data::Object::new().with("agreeing", self.agreeing).with("total", self.total)
    }
}

impl Status for Compared {
    fn verb(&self) -> Verb {
        if self.agreeing == self.total { Verb::good("Agreed") } else { Verb::failed("Compared") }
    }
}
