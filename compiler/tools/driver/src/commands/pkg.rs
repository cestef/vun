//! `vun pkg`: the project's dependencies and releases (spec 12, sections 3 and 7).

use std::io::{self, Write};
use std::path::PathBuf;

use vun_build::layout::MANIFEST;
use vun_build::manifest::{Source, Version};
use vun_command::{Action, Args, Command, Outcome, Param, ParamKind};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fs::Edit;
use vun_pkg::{FORMATS, Format};

use crate::Session;
use crate::packages::{Addition, Fetched, Unstated};

/// The group of these commands.
pub(super) const GROUP: &str = "pkg";

pub(super) const FETCH: Command<Session> = Command {
    name: "fetch",
    about: "download dependencies into the local store and write the lockfile",
    params: &[],
    action: Action::Run(fetch),
};
/// Where `vun pkg fetch` is below the root, for messages that suggest it.
pub(crate) const FETCH_PATH: [&str; 2] = [GROUP, FETCH.name];

pub(super) const SOURCE: Param = Param {
    name: "source",
    short: None,
    about: "a package directory, or the URL of an archive",
    kind: ParamKind::Positional,
};
pub(super) const NAME: Param = Param {
    name: "name",
    short: None,
    about: "the name to declare it under, when the package states none",
    kind: ParamKind::Option,
};
pub(super) const VERSION: Param = Param {
    name: "version",
    short: None,
    about: "its version, when the package states none",
    kind: ParamKind::Option,
};
pub(super) const OUT: Param = Param {
    name: "out",
    short: Some('o'),
    about: "where to write the archive; its suffix picks the format",
    kind: ParamKind::Option,
};

/// `vun pkg fetch`: downloads the stored dependencies of the project into its store.
fn fetch(session: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let Some((dir, project)) = session.project()? else { return Ok(Outcome::Failure) };
    for (name, hash, fetched) in session.fetch(project) {
        let name = name.as_str();
        match fetched {
            Fetched::Present => writeln!(out, "present {name} {hash}")?,
            Fetched::Stored { url } => writeln!(out, "stored  {name} {hash} from {url}")?,
            Fetched::Failed => {}
        }
    }
    if !session.failed() {
        session.write_lock(&dir, project);
    }
    Ok(session.outcome())
}

/// `vun pkg add <source>`: declares a dependency, fetching an archive once to learn its hash.
pub(super) fn add(session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let Some((dir, _)) = session.project()? else { return Ok(Outcome::Failure) };
    let version = match args.value(&VERSION).map(|v| (v, Version::parse(v))) {
        Some((text, None)) => {
            let msg = Msg::new().code(text).text(" is not a version ").code("major.minor.patch");
            session.diag.error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg));
            return Ok(Outcome::Usage);
        }
        parsed => parsed.and_then(|(_, v)| v),
    };
    let source = args.value(&SOURCE).expect("positional parameters are present when a command runs");
    let addition = Addition { source, name: args.value(&NAME), version };
    match session.add(&dir, &addition) {
        Ok(Some(dep)) => {
            match &dep.source {
                Source::Path(path) => writeln!(out, "added {} at {path}", dep.name)?,
                Source::Archive { version, hash, .. } => {
                    writeln!(out, "added {} {version} {hash}", dep.name)?;
                }
            }
            if let Some(project) = session.package_at(&dir) {
                session.write_lock(&dir, project);
            }
        }
        Ok(None) => {}
        Err(unstated) => {
            let param = match unstated {
                Unstated::Name => &NAME,
                Unstated::Version => &VERSION,
            };
            let msg = Msg::new().code(source).text(" has no ").code(MANIFEST).text(" stating ");
            let help = Msg::new().text("give it with ").code(param.long());
            let d = Diagnostic::error(errors::PACKAGE_UNNAMED, msg.text(unstated.to_string()));
            session.diag.error(d.child(ChildKind::Help, help));
        }
    }
    Ok(session.outcome())
}

/// `vun pkg update`: raises the stored dependencies of the project within their majors, then
/// rewrites the lockfile.
pub(super) fn update(session: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let Some((dir, _)) = session.project()? else { return Ok(Outcome::Failure) };
    let updated = session.update(&dir);
    for u in &updated {
        writeln!(out, "updated {} {} to {} {}", u.name, u.from, u.to, u.hash)?;
    }
    if updated.is_empty() && !session.failed() {
        writeln!(out, "every dependency is at the newest release its indexes list")?;
    }
    if let Some(project) = session.package_at(&dir) {
        session.fetch(project);
        if !session.failed() {
            session.write_lock(&dir, project);
        }
    }
    Ok(session.outcome())
}

/// `vun pkg pack`: writes the project's archive, named after its release, in the published
/// format unless `--out` names another format by its suffix.
pub(super) fn pack(session: &mut Session, args: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let Some((dir, _)) = session.project()? else { return Ok(Outcome::Failure) };
    let Some(manifest) = session.manifest_at(&dir.join(MANIFEST)) else { return Ok(Outcome::Failure) };
    let release = manifest.release();
    let path = args
        .value(&OUT)
        .map_or_else(|| dir.join([release.as_str(), Format::PUBLISHED.suffix()].concat()), PathBuf::from);
    let Some(format) = Format::of(&path.to_string_lossy()) else {
        let suffixes = FORMATS.iter().flat_map(|f| f.suffixes.iter().copied());
        let msg = Msg::new().code(path.display().to_string()).text(" names no archive format; end it in ");
        session.diag.error(Diagnostic::error(errors::CLI_INVALID_VALUE, msg.code_list(suffixes)));
        return Ok(Outcome::Usage);
    };
    let (bytes, snapshot) = match format.pack(&dir, &manifest.include, &release) {
        Ok(packed) => packed,
        Err(e) => {
            let msg = Msg::new().text("cannot pack ").code(&manifest.name);
            session.diag.error(Diagnostic::error(errors::IO_UNREADABLE_FILE, msg).caused_by(&e));
            return Ok(Outcome::Failure);
        }
    };
    if let Err(e) = path.put(&bytes) {
        let msg = Msg::new().text("cannot write ").code(path.display().to_string());
        session.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e));
        return Ok(Outcome::Failure);
    }
    writeln!(out, "packed {} {} {} {}", manifest.name, manifest.version, snapshot.root(), path.display())?;
    Ok(session.outcome())
}
