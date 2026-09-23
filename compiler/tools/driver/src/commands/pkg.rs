//! `overt pkg`: the project's dependencies (spec 12, sections 3 and 7).

use std::io::{self, Write};

use overt_build::store::Store;
use overt_command::{Args, Outcome};
use overt_diag::{Diagnostic, Msg, errors};

use crate::Session;
use crate::packages::Fetched;

/// `overt pkg fetch`: downloads the stored dependencies of the project around the current
/// directory into its store.
pub(super) fn fetch(session: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let here = std::env::current_dir()?;
    let Some((dir, project)) =
        here.ancestors().find_map(|d| session.package_at(d).map(|p| (d.to_path_buf(), p)))
    else {
        let msg =
            Msg::new().text("no ").code(overt_build::layout::MANIFEST).text(" declaring ").code("package");
        let msg = msg.text(" in this directory or above");
        session.diag.error(Diagnostic::error(errors::PACKAGE_NO_PROJECT, msg));
        return Ok(Outcome::Failure);
    };
    session.store = Some(Store::in_project(&dir));
    for (name, hash, fetched) in session.fetch(project) {
        let name = session.interner.resolve(name);
        match fetched {
            Fetched::Present => writeln!(out, "present {name} {hash}")?,
            Fetched::Stored { url } => writeln!(out, "stored  {name} {hash} from {url}")?,
            Fetched::Failed => {}
        }
    }
    Ok(if session.failed() { Outcome::Failure } else { Outcome::Success })
}
