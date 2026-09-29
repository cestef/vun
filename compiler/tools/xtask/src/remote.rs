//! `cargo xtask remote`: runs a command on another machine, over the working copy as it is here.
//! The repository is mirrored to `vun-build/<its directory's name>` in the host's home, one
//! mirror per `jj` workspace, each building into its own `target/`; the command then runs there,
//! its output shown as it is written.

use std::io;
use std::path::Path;

use vun_command::{Args, Outcome, Param, Shell};
use vun_data::Object;
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_harness::repo_root;
use vun_intern::Symbol;
use vun_term::{Record, Status, Style, Text, Verb};
use vun_tool::{Arg, Failure, Invocation, Streams, Tool};

use crate::task::{Program, WORDS};

pub(crate) const HOST: Param =
    Param::option("host").env("VUN_REMOTE").about("the machine to run on, as ssh names it");

/// Where mirrors live in the host's home.
const MIRRORS: &str = "vun-build";
/// What stays here: builds and the repository's history, anchored at its root, since crates are
/// named `target` too.
const EXCLUDED: [&str; 2] = ["/target", "/.jj"];
/// `rsync` mirroring a directory, creating the destination's parents.
const MIRROR: [&str; 3] = ["--archive", "--delete", "--mkpath"];
/// What the remote shell runs before the command: the toolchain rustup installed on the host.
const PRELUDE: [&str; 3] = [". ~/.cargo/env", "&&", "cd"];
const THEN: &str = "&&";

const NO_HOST: DiagId = DiagId::new("xtask.remote.no_host");
const UNSYNCED: DiagId = DiagId::new("xtask.remote.unsynced");
const FAILED: DiagId = DiagId::new("xtask.remote.failed");

#[expect(clippy::unnecessary_wraps, reason = "every command has the `Run` signature")]
pub(crate) fn run(diag: &mut DiagCtxt, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some(host) = args.value(&HOST) else {
        let msg =
            Msg::new().text("no host to run on: pass ").code("--host").text(" or set ").code("VUN_REMOTE");
        diag.error(Diagnostic::error(NO_HOST, msg));
        return Ok(Outcome::Failure);
    };
    let root = repo_root();
    let mirror = Mirror { host, dir: Path::new(MIRRORS).join(root.file_name().unwrap_or_default()) };
    let result = mirror.sync(&root).and_then(|()| {
        shell.status(&Synced { host });
        mirror.run(args.values(&WORDS))
    });
    Ok(match result {
        Ok(()) => Outcome::Success,
        Err(d) => {
            diag.error(*d);
            Outcome::Failure
        }
    })
}

/// The copy of the repository on a host.
struct Mirror<'h> {
    host: &'h str,
    /// Relative to the host's home.
    dir: std::path::PathBuf,
}

impl Mirror<'_> {
    /// Brings the mirror up to date with the working copy at `root`.
    fn sync(&self, root: &Path) -> Result<(), Box<Diagnostic>> {
        let mut args: Vec<Arg> = MIRROR.into_iter().map(Arg::from).collect();
        for excluded in EXCLUDED {
            args.extend([Arg::from("--exclude"), Arg::from(excluded)]);
        }
        // A trailing `/` copies the directory's contents rather than the directory.
        args.push(Arg::Path(root.join("")));
        args.push(Arg::Text(Symbol::intern(&format!("{}:{}/", self.host, self.dir.display()))));
        let msg = Msg::new().text("cannot mirror the working copy to ").code(self.host);
        Self::call(Program::Rsync, args, Streams::Captured, Diagnostic::error(UNSYNCED, msg))
    }

    /// Runs `words` in the mirror, their output shown as it comes.
    fn run<'w>(&self, words: impl Iterator<Item = &'w str>) -> Result<(), Box<Diagnostic>> {
        let mut args = vec![Arg::Text(Symbol::intern(self.host))];
        args.extend(PRELUDE.into_iter().map(Arg::from));
        args.extend([Arg::Path(self.dir.clone()), Arg::from(THEN)]);
        args.extend(words.map(|w| Arg::Text(Symbol::intern(w))));
        let msg = Msg::new().text("the command failed on ").code(self.host);
        Self::call(Program::Ssh, args, Streams::Inherited, Diagnostic::error(FAILED, msg))
    }

    /// Runs `program` with `args`; `failed` reports it failing, caused by how.
    fn call(
        program: Program,
        args: Vec<Arg>,
        streams: Streams,
        failed: Diagnostic,
    ) -> Result<(), Box<Diagnostic>> {
        let failure = match program.find()?.run(&Invocation { args, streams, ..Invocation::default() }) {
            Ok(out) => match out.failure() {
                None => return Ok(()),
                Some(failure) => failure,
            },
            Err(e) => Failure::Start(e),
        };
        Err(Box::new(failed.caused_by(&failure)))
    }
}

/// The working copy, mirrored to a host.
struct Synced<'h> {
    host: &'h str,
}

impl Record for Synced<'_> {
    fn kind(&self) -> &'static str {
        "remote"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.host.to_owned())
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("host", self.host.to_owned())
    }
}

impl Status for Synced<'_> {
    fn verb(&self) -> Verb {
        Verb::good("Synced")
    }
}
