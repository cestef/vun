//! `vun pkg`: the project's dependencies and releases (spec 12, sections 3 and 7).

use std::io;
use std::path::PathBuf;

use vun_build::layout::MANIFEST;
use vun_build::manifest::{Source, Version};
use vun_command::{Args, Command, Example, Outcome, Param, PathKind, Values};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fs::Edit;
use vun_pkg::{FORMATS, Format};
use vun_term::{Shell, Style, Text};

use super::status::{ADDED, FRESH, HashText, PACKED, Shown, UPDATED};
use crate::Session;
use crate::packages::{Addition, Unstated};

/// The group of these commands.
pub(super) const GROUP: &str = "pkg";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run("add", "declare a dependency, fetching an archive to learn its hash", add)
        .params(&[SOURCE, NAME, VERSION])
        .examples(&[
            Example { line: "../geometry", about: "a package directory" },
            Example { line: "https://example.org/geometry-1.2.0.tar.zst", about: "a released archive" },
        ]),
    Command::run("update", "raise dependencies to the newest release their indexes list", update),
    Command::run("pack", "write the package's archive", pack).params(&[OUT]),
];

const SOURCE: Param = Param::positional("source").about("a package directory, or the URL of an archive");
const NAME: Param = Param::option("name").about("the name to declare it under, when the package states none");
const VERSION: Param = Param::option("version").about("its version, when the package states none");
const OUT: Param = Param::option("out")
    .short('o')
    .about("where to write the archive; its suffix picks the format")
    .values(Values::Path(PathKind::File));

/// `vun pkg add <source>`: declares a dependency, fetching an archive once to learn its hash.
fn add(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
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
            let name = Text::of(Style::Strong, dep.name.clone());
            let what = match &dep.source {
                Source::Path(path) => name.plain(" at ").plain(path.clone()),
                Source::Archive { version, hash, .. } => {
                    name.plain(format!(" {version} ")).append(hash.text(Shown::Short))
                }
            };
            shell.status(ADDED, what);
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
fn update(session: &mut Session, _: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
    let Some((dir, _)) = session.project()? else { return Ok(Outcome::Failure) };
    let updated = session.update(&dir);
    for u in &updated {
        let what = Text::of(Style::Strong, u.name.clone()).plain(format!(" {} to {} ", u.from, u.to));
        shell.status(UPDATED, what.append(u.hash.text(Shown::Short)));
    }
    if updated.is_empty() && !session.failed() {
        shell.status(FRESH, Text::from("every dependency is at the newest release its indexes list"));
    }
    if let Some(project) = session.package_at(&dir) {
        for (name, hash, fetched) in session.fetch(project) {
            fetched.report(name, hash, shell);
        }
        if !session.failed() {
            session.write_lock(&dir, project);
        }
    }
    Ok(session.outcome())
}

/// `vun pkg pack`: writes the project's archive, named after its release, in the published
/// format unless `--out` names another format by its suffix.
fn pack(session: &mut Session, args: &Args, shell: &mut Shell<'_>) -> io::Result<Outcome> {
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
    let what = Text::of(Style::Strong, format!("{} {}", manifest.name, manifest.version)).plain(" ");
    shell.status(
        PACKED,
        what.append(snapshot.root().text(Shown::Short)).plain(" to ").plain(path.display().to_string()),
    );
    Ok(session.outcome())
}
