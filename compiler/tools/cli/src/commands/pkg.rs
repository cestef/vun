//! `vun pkg`: the project's dependencies and releases (spec 12, sections 3 and 7).

use std::io;
use std::path::PathBuf;

use vun_build::layout::MANIFEST;
use vun_build::manifest::{Source, Version};
use vun_command::{Args, Command, CommandVerb, Example, Outcome, Param, PathKind, Values};
use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_fetch::{FORMATS, Format};
use vun_fs::Edit;
use vun_term::Shell;

use super::status::{self, Report, Source as Added};
use vun_driver::Session;
use vun_driver::packages::{Addition, Unstated};

/// The group of these commands.
pub(super) const GROUP: &str = "pkg";

pub(super) const COMMANDS: &[Command<Session>] = &[
    Command::run(CommandVerb::Add, "declare a dependency, fetching an archive to learn its hash", add)
        .params(&[SOURCE, NAME, VERSION])
        .examples(&[
            Example { line: "../geometry", about: "a package directory" },
            Example { line: "https://example.org/geometry-1.2.0.tar.zst", about: "a released archive" },
        ]),
    Command::run(CommandVerb::Update, "raise dependencies to the newest release their indexes list", update),
    Command::run(CommandVerb::Pack, "write the package's archive", pack).params(&[OUT]),
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
    let version = match args.value(&VERSION).map(|v| (v, v.parse::<Version>().ok())) {
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
            let source = match &dep.source {
                Source::Path(path) => Added::Path(path.clone()),
                Source::Library(path) => Added::Library(path.clone()),
                Source::Archive { version, hash, .. } => Added::Archive { version: *version, hash: *hash },
            };
            shell.status(&status::Added { name: dep.name.clone(), source });
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
        shell.status(&status::Updated { name: u.name.clone(), from: u.from, to: u.to, hash: u.hash });
    }
    if updated.is_empty() && !session.failed() {
        shell.status(&status::UpToDate);
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
        session.diag.error(Diagnostic::unwritable(&path, &e));
        return Ok(Outcome::Failure);
    }
    let path = path.display().to_string();
    let hash = snapshot.root();
    shell.status(&status::Packed { name: manifest.name.clone(), version: manifest.version, hash, path });
    Ok(session.outcome())
}
