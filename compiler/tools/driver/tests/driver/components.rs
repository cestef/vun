//! Installing the components of a release (spec 12, section 6): each library its own archive,
//! checked against the manifest's hash before it is put in the toolchain's `components`
//! directory, and put again when it changed since.

use vun_build::release::{Build, Piece, Profile, Published, Release, Releases};
use vun_build::toolchains::{COMPONENTS, NAME, Toolchains};
use vun_data::ToData;
use vun_driver::install::{Installed, Uninstalled};
use vun_fetch::Format;
use vun_fs::Edit;
use vun_hash::Hash;
use vun_span::Version;
use vun_target::Machine;
use vun_uri::Uri;

use crate::common::Fixture;

const LIBRARY: &str = "libvun_component_backend_x.so";

/// A release of `version` laid out as a forge lays it out, with one component whose library
/// holds `bytes`; the manifest's hash for it is `claimed` when given.
struct Laid {
    fixture: Fixture,
    releases: Releases,
    published: Published,
    toolchains: Toolchains,
    version: Version,
}

impl Laid {
    fn new(name: &str, bytes: &[u8], claimed: Option<Hash>) -> Self {
        let fixture = Fixture::new(name);
        let version = Version::parse("0.4.2").unwrap();
        // The component's archive: a tree holding the library alone.
        let tree = fixture.root.path().join("tree");
        tree.join(COMPONENTS).join(LIBRARY).put(bytes).unwrap();
        let format = Format::of(Build::SUFFIX).unwrap();
        let (archive, snapshot) = format.pack(&tree, &[COMPONENTS.to_owned()], NAME).unwrap();
        let machine = Machine::RUNNING;
        let piece = Piece {
            name: "backend-x".into(),
            archive: Piece::archive_name("backend-x", machine),
            file: LIBRARY.into(),
            hash: claimed.unwrap_or_else(|| snapshot.root()),
            profile: Profile::Default,
            needs: Vec::new(),
        };
        let published = Self::publish(machine, version, piece.clone());
        let base = fixture.root.path().join("forge");
        base.join("releases").join("download").join("0.4.2").join(&piece.archive).put(archive).unwrap();
        let releases = Releases::at(Uri::from_file_path(&base).unwrap(), vun_build::release::Key::releases());
        // The toolchain the component is installed into, installed already.
        let toolchains =
            Toolchains::at(&fixture.root.path().join("toolchains"), &fixture.root.path().join("vun"));
        toolchains.dir(version).join("bin").join("vun").put(b"vun").unwrap();
        Self { fixture, releases, published, toolchains, version }
    }

    fn publish(machine: Machine, version: Version, piece: Piece) -> Published {
        let build = Build {
            machine,
            archive: Build::archive_name(machine),
            hash: Hash::of(b"tree"),
            components: vec![piece],
        };
        let release = Release { version, builds: vec![build] };
        let manifest = vun_data::json::Json(&release.to_data()).to_string().into_bytes();
        Published { release, manifest }
    }

    fn install(&self) -> Result<Vec<(String, Installed)>, Uninstalled> {
        let session = self.fixture.session();
        let names = Session::profile_components(&self.published, Profile::Default)?;
        session.install_components(&self.toolchains, &self.releases, &self.published, &names)
    }

    fn library(&self) -> Option<Vec<u8>> {
        std::fs::read(self.toolchains.components(self.version).join(LIBRARY)).ok()
    }
}

use vun_driver::Session;

#[test]
fn a_component_is_fetched_checked_and_installed_once() {
    let laid = Laid::new("components-install", b"library", None);
    assert_eq!(laid.install().unwrap(), [("backend-x".to_owned(), Installed::Fresh)]);
    assert_eq!(laid.library().as_deref(), Some(&b"library"[..]));
    assert_eq!(laid.install().unwrap(), [("backend-x".to_owned(), Installed::Already)]);
}

#[test]
fn a_library_changed_since_is_installed_again() {
    let laid = Laid::new("components-changed", b"library", None);
    laid.install().unwrap();
    laid.toolchains.components(laid.version).join(LIBRARY).put(b"tampered").unwrap();
    assert_eq!(laid.install().unwrap()[0].1, Installed::Fresh);
    assert_eq!(laid.library().as_deref(), Some(&b"library"[..]));
}

#[test]
fn an_archive_that_is_not_the_one_the_manifest_names_installs_nothing() {
    let laid = Laid::new("components-forged", b"library", Some(Hash::of(b"another")));
    assert!(matches!(laid.install(), Err(Uninstalled::Changed { .. })));
    assert_eq!(laid.library(), None);
}

#[test]
fn the_profiles_hold_the_components_of_their_own_and_smaller_ones() {
    let laid = Laid::new("components-profiles", b"library", None);
    let names = |profile| Session::profile_components(&laid.published, profile).unwrap();
    assert!(names(Profile::Minimal).is_empty());
    assert_eq!(names(Profile::Default), ["backend-x"]);
    assert_eq!(names(Profile::Full), ["backend-x"]);
}
