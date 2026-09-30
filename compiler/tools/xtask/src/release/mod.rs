//! `cargo xtask package` and `cargo xtask release`: what a release of the toolchain is made of
//! (spec 12, section 6). `package` puts a built `vun` beside the repository's library as a
//! toolchain's tree, packs it as the release archive of the machine it runs on and records the
//! tree's hash beside it; `release` gathers what `package` wrote on each machine into the
//! release's manifest.

use std::fs;
use std::io;
use std::path::Path;

use vun_build::release::{Build, Release};
use vun_build::toolchains::{BIN, CONTENTS, NAME, Toolchains};
use vun_command::{Args, Outcome, Param, PathKind, Shell, Values};
use vun_data::json::Json;
use vun_data::{FromData, ToData};
use vun_diag::{DiagCtxt, DiagId, Diagnostic, Msg};
use vun_fetch::Format;
use vun_fs::{Edit, Tree};
use vun_harness::repo_root;
use vun_span::Version;
use vun_target::Machine;

const UNPACKED: DiagId = DiagId::new("xtask.release.unpacked");
const UNTAGGED: DiagId = DiagId::new("xtask.release.untagged");

pub(crate) const BINARY: Param =
    Param::positional("binary").about("the `vun` built for the release").values(Values::Path(PathKind::File));
pub(crate) const OUT: Param = Param::positional("out")
    .about("the directory the archives and their builds are written in")
    .values(Values::Path(PathKind::Dir));
pub(crate) const TAG: Param =
    Param::option("tag").about("the tag of the release, which must be the workspace's version");

/// The extension of the file recording a packed archive's build beside it.
const BUILD: &str = "json";
/// Where the toolchain's tree is put together in the output directory before it is packed.
const STAGE: &str = ".stage";

pub(crate) fn package(diag: &mut DiagCtxt, args: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    let binary = Path::new(args.value(&BINARY).expect("positional parameters are present"));
    let out = Path::new(args.value(&OUT).expect("positional parameters are present"));
    let stage = out.join(STAGE);
    stage.fresh()?;
    let root = repo_root();
    let exe = stage.join(BIN).join(binary.file_name().unwrap_or_default());
    exe.parent().map(fs::create_dir_all).transpose()?;
    fs::copy(binary, &exe)?;
    for part in CONTENTS.iter().filter(|&&c| c != BIN) {
        for file in root.join(part).files()? {
            let relative = file.strip_prefix(&root).unwrap_or(&file);
            stage.join(relative).put(fs::read(&file)?)?;
        }
    }
    let contents: Vec<String> = CONTENTS.iter().map(|&c| c.to_owned()).collect();
    let format = Format::of(Build::SUFFIX).expect("toolchain archives are of a known format");
    let (bytes, snapshot) = format.pack(&stage, &contents, NAME)?;
    let archive = Build::archive_name(Machine::RUNNING);
    let build = Build { machine: Machine::RUNNING, archive: archive.clone(), hash: snapshot.root() };
    let hashed = Toolchains::hash(&stage)?;
    stage.remove()?;
    if hashed != build.hash {
        diag.error(Diagnostic::error(
            UNPACKED,
            Msg::new().text("the staged toolchain hashes apart from its snapshot"),
        ));
        return Ok(Outcome::Failure);
    }
    out.join(&archive).put(bytes)?;
    out.join(&archive).with_added_extension(BUILD).put(Json(&build.to_data()).to_string())?;
    Ok(Outcome::Success)
}

pub(crate) fn release(diag: &mut DiagCtxt, args: &Args, _: &mut Shell<'_>) -> io::Result<Outcome> {
    let out = Path::new(args.value(&OUT).expect("positional parameters are present"));
    let version = Version::parse(env!("CARGO_PKG_VERSION")).expect("the workspace's version is one");
    if let Some(tag) = args.value(&TAG)
        && Version::parse(tag) != Some(version)
    {
        let msg = Msg::new()
            .text("the tag ")
            .code(tag)
            .text(" is not the workspace's version ")
            .code(version.to_string());
        diag.error(Diagnostic::error(UNTAGGED, msg));
        return Ok(Outcome::Failure);
    }
    let mut builds = Vec::new();
    for file in out.files_with(BUILD)? {
        if file.file_name().is_some_and(|n| n == Release::FILE) {
            continue;
        }
        let text = fs::read_to_string(&file)?;
        let value = vun_data::json::parse(&text).map_err(io::Error::other)?;
        builds.push(Build::from_data(&value).map_err(io::Error::other)?);
    }
    let manifest = Release { version, builds };
    out.join(Release::FILE).put(Json(&manifest.to_data()).to_string())?;
    Ok(Outcome::Success)
}
