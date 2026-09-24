//! `overt pkg update`: a dependency is raised to the newest release its index lists in its major,
//! verified against the listed hash, and its manifest entry rewritten.

use crate::common::Fixture;
use overt_build::index::{Index, Listing};
use overt_build::layout::MANIFEST;
use overt_build::manifest::Identity;
use overt_diag::errors;
use overt_hash::Hash;
use overt_pkg::{Format, Scheme};

const HAL: Identity = Identity { id: 7, major: 1 };

/// Publishes `hal` at `version`, holding `source`, into the index at `index`, listed with `listed`
/// as its hash (its real one when `None`).
fn release(fx: &Fixture, index: &str, version: &str, source: &str, listed: Option<Hash>) -> Hash {
    let hal = Fixture::manifest("hal", HAL.id, version);
    let dir = fx.write(&format!("hal-{version}"), &hal, source);
    let (bytes, snapshot) = Format::PUBLISHED.pack(&dir, &hal.include, &hal.release()).unwrap();
    let base = Scheme::file_url(&fx.root.join(index));
    let archive = Index { base: &base }.archive(HAL, hal.version);
    fx.root.write(archive.strip_prefix(Scheme::FILE.prefix).unwrap(), bytes);
    let listing = Listing { releases: vec![(hal.version, listed.unwrap_or(snapshot.root()))] };
    let list = Index { base: &base }.listing(HAL);
    fx.root.write(list.strip_prefix(Scheme::FILE.prefix).unwrap(), listing.to_string());
    snapshot.root()
}

#[test]
fn dependencies_are_raised_within_their_major() {
    let fx = Fixture::new("update");
    let hal = Fixture::manifest("hal", HAL.id, "1.0.0");
    fx.write("hal", &hal, "a := 1;\n");
    let (url, hash) = fx.publish("hal", &hal);
    let newer = release(&fx, "index", "1.2.0", "a := 1;\nb := 2;\n", None);
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.prelude.push("hal".into());
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, hash));
    app.index.push(Scheme::file_url(&fx.root.join("index")));
    let dir = fx.write("app", &app, "x := b;\n");

    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    session.fetch(project);
    let updated = session.update(&dir);
    assert!(!session.failed(), "{:?}", session.diag.sorted());
    let raised: Vec<_> = updated.iter().map(|u| (u.from.to_string(), u.to.to_string(), u.hash)).collect();
    assert_eq!(raised, [("1.0.0".into(), "1.2.0".into(), newer)]);
    let text = std::fs::read_to_string(dir.join(MANIFEST)).unwrap();
    assert!(text.contains("\"1.2.0\"") && text.contains(&newer.to_string()), "{text}");
    assert_eq!(fx.check(&Fixture::file(&dir, &app)), []);
}

#[test]
fn a_release_not_matching_its_listing_is_refused() {
    let fx = Fixture::new("update-tampered");
    let hal = Fixture::manifest("hal", HAL.id, "1.0.0");
    fx.write("hal", &hal, "a := 1;\n");
    let (url, hash) = fx.publish("hal", &hal);
    release(&fx, "index", "1.2.0", "a := 1;\n", Some(Hash::of(b"another")));
    let mut app = Fixture::manifest("app", 1, "0.1.0");
    app.deps.push(Fixture::stored("hal", "1.0.0", &url, hash));
    app.index.push(Scheme::file_url(&fx.root.join("index")));
    let dir = fx.write("app", &app, "");
    let mut session = fx.session();
    let project = session.package_at(&dir).unwrap();
    session.fetch(project);
    assert!(session.update(&dir).is_empty());
    let ids: Vec<_> = session.diag.sorted().into_iter().map(|d| d.id.clone()).collect();
    assert_eq!(ids, [errors::PACKAGE_FETCH_FAILED]);
}
