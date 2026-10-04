//! `cargo xtask tree-sitter-publish <message>`: the grammar of `editors/tree-sitter` published to
//! its repository. What `Release::PUBLISHED` names is copied into the repository's clone, which is
//! committed and pushed there; the commit it makes is recorded beside the grammar
//! (`release.json`), and the editors' packages are generated to build it.

use std::io;
use std::path::{Path, PathBuf};

use vun_command::{Args, Outcome, Param, PathKind, Shell, Values};
use vun_data::json::Json;
use vun_data::{Object, ToData};
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fs::{Edit, Tree as _};
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Executable, Failure, Invocation, Streams, Tool};
use vun_tree_sitter::Release;

use super::GRAMMAR;
use crate::task::{GENERATE, Program};

const FAILED: DiagId = DiagId::new("xtask.tree_sitter.publish_failed");

pub(crate) const MESSAGE: Param =
    Param::positional("message").about("the commit's message in the grammar's repository");
pub(crate) const REPO: Param = Param::option("repo")
    .about("the clone of the grammar's repository; `tree-sitter-vun` beside this repository if none")
    .values(Values::Path(PathKind::Dir));

/// The clone's directory beside the repository's own.
const CLONE: &str = "tree-sitter-vun";
/// The branch the packages build from, which a publication moves.
const BRANCH: &str = "main";

/// The grammar's repository, cloned.
struct Repository {
    dir: PathBuf,
    jj: Executable,
}

impl Repository {
    /// Runs `jj` with `words` in the clone, a push as the person watching it.
    fn jj(&self, words: &[&str], streams: Streams) -> Result<String, Box<Diagnostic>> {
        let inv = Invocation {
            args: words.iter().map(|w| Arg::Text(Symbol::intern(w))).collect(),
            cwd: Some(self.dir.clone()),
            streams,
            ..Invocation::default()
        };
        let line = || ["jj ", &words.join(" ")].concat();
        let failed = |why: &Failure| {
            let msg = Msg::new().text("could not run ").code(line());
            Box::new(Diagnostic::error(FAILED, msg).caused_by(why))
        };
        let out = self.jj.run(&inv).map_err(|e| failed(&Failure::Start(e)))?;
        match out.failure() {
            None => Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned()),
            Some(why) => Err(failed(&why)),
        }
    }

    /// Makes what the repository holds of the grammar what `from` holds: each published entry
    /// replaced whole, so a file the grammar no longer has is gone.
    fn mirror(&self, from: &Path) -> io::Result<()> {
        for name in Release::PUBLISHED {
            let (source, target) = (from.join(name), self.dir.join(name));
            if !source.is_dir() {
                target.put(std::fs::read(&source)?)?;
                continue;
            }
            target.remove()?;
            for file in source.files()? {
                let relative = file.strip_prefix(&source).unwrap_or(&file);
                target.join(relative).put(std::fs::read(&file)?)?;
            }
        }
        Ok(())
    }

    /// Commits what changed with `message` and pushes it; the commit the branch is at, whether
    /// or not anything changed.
    fn publish(&self, message: &str) -> Result<String, Box<Diagnostic>> {
        if !self.jj(&["diff", "--summary"], Streams::Captured)?.is_empty() {
            self.jj(&["commit", "-m", message], Streams::Captured)?;
            self.jj(&["bookmark", "set", BRANCH, "-r", "@-"], Streams::Captured)?;
            self.jj(&["git", "push", "--bookmark", BRANCH], Streams::Inherited)?;
        }
        self.jj(&["log", "-r", BRANCH, "--no-graph", "-T", "commit_id"], Streams::Captured)
    }
}

pub(crate) fn publish(diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let root = repo_root();
    let message = args.value(&MESSAGE).expect("positional parameters are present when a command runs");
    let dir = args.value(&REPO).map_or_else(|| root.join("..").join(CLONE), PathBuf::from);
    let published = Program::Jj.find().and_then(|jj| {
        let repository = Repository { dir, jj };
        repository.mirror(&root.join(GRAMMAR)).map_err(|e| {
            let msg = Msg::new().text("could not copy the grammar into the clone");
            Box::new(Diagnostic::error(FAILED, msg).caused_by(&e))
        })?;
        repository.publish(message)
    });
    let revision = match published {
        Ok(revision) => revision,
        Err(d) => {
            diag.error(*d);
            return Ok(Outcome::Failure);
        }
    };
    let release = Release { revision };
    root.join(GRAMMAR).join(Release::FILE).put(format!("{}\n", Json(&release.to_data())))?;
    shell.status(&Published { revision: release.revision });
    // The editors' packages build the revision the grammar records.
    GENERATE.run(diag, args, shell)
}

/// The commit of the grammar's repository the editors build.
struct Published {
    revision: String,
}

impl Record for Published {
    fn kind(&self) -> &'static str {
        "tree-sitter"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.revision.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("revision", self.revision.clone())
    }
}

impl Status for Published {
    fn verb(&self) -> Verb {
        Verb::good("Published")
    }
}

#[cfg(test)]
mod tests;
