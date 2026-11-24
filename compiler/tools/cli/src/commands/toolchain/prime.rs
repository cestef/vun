//! `vun toolchain prime`: the libraries' memos the running toolchain ships, made by checking the
//! libraries whole (decision 667).

use std::io;

use vun_build::toolchains::MEMOS;
use vun_command::{Args, Command, CommandVerb, Outcome, Param, PathKind, Values};
use vun_data::Object;
use vun_diag::{Diagnostic, Msg, errors};
use vun_driver::Session;
use vun_driver::packages::Library;
use vun_driver::prime::{Prime, Primed};
use vun_term::{Bytes, Record, Shell, Status, Text, Verb};

const INTO: Param = Param::option("into")
    .about("the directory to keep the memos in; the toolchain's own `memos` if not given")
    .values(Values::Path(PathKind::Dir));

pub(super) const PRIME: Command<Session> = Command::run(
    CommandVerb::Prime,
    "check the libraries whole and ship their memos with this toolchain, so that a first check finds them",
    prime,
)
.params(&[INTO]);

/// The memos kept.
struct Kept(Primed);

impl Record for Kept {
    fn kind(&self) -> &'static str {
        "primed"
    }

    fn text(&self) -> Text {
        Text::from(format!("{} files checked, {} of memos", self.0.files, Bytes(self.0.bytes)))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("files", self.0.files).with("bytes", self.0.bytes)
    }
}

impl Status for Kept {
    fn verb(&self) -> Verb {
        Verb::good("Primed")
    }
}

fn prime(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let memos = match args.value(&INTO) {
        Some(dir) => dir.into(),
        None => vun_fs::EXECUTABLE
            .as_deref()
            .and_then(|exe| exe.ancestors().nth(2))
            .ok_or(io::ErrorKind::NotFound)?
            .join(MEMOS),
    };
    match (Prime { library: Library::find().dir, memos }).run() {
        Ok(primed) => {
            session.diag.absorb(&primed.diag);
            shell.status(&Kept(primed));
            Ok(session.outcome())
        }
        Err(e) => {
            let msg = Msg::new().text("cannot prime the toolchain");
            session.diag.error(Diagnostic::error(errors::TOOLCHAIN_PRIME_FAILED, msg).caused_by(&e));
            Ok(Outcome::Failure)
        }
    }
}
