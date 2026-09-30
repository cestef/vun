use vun_data::{FromData, ToData};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::{Machine, Name};
use vun_uri::Uri;

use ring::signature::{Ed25519KeyPair, KeyPair};

use super::{Build, Key, Release, Releases, Wanted};

fn release() -> Release {
    let machine = Machine { arch: Name("x86_64"), os: Name("linux") };
    let build = Build { machine, archive: Build::archive_name(machine), hash: Hash::of(b"tree") };
    Release { version: Version::parse("0.4.2").unwrap(), builds: vec![build] }
}

#[test]
fn a_manifest_reads_back_as_it_was_written() {
    let release = release();
    let text = vun_data::json::Json(&release.to_data()).to_string();
    let read = Release::from_data(&vun_data::json::parse(&text).unwrap()).unwrap();
    assert_eq!(read, release);
    assert!(read.build_for(Machine { arch: Name("x86_64"), os: Name("linux") }).is_some());
    assert!(read.build_for(Machine { arch: Name("arm64"), os: Name("macos") }).is_none());
}

#[test]
fn releases_are_found_as_a_forge_lays_them_out() {
    let releases = Releases::at(Uri::parse("https://forge.example/owner/vun").unwrap(), Key::releases());
    let release = release();
    let path =
        |uri: Uri| uri.path().segments().map(|s| s.decode_utf8().unwrap().into_owned()).collect::<Vec<_>>();
    assert_eq!(
        path(releases.manifest(Wanted::Latest)),
        ["owner", "vun", "releases", "latest", "download", Release::FILE]
    );
    assert_eq!(
        path(releases.archive(&release, &release.builds[0])),
        ["owner", "vun", "releases", "download", "0.4.2", "vun-x86_64-linux.tar.gz"]
    );
}

#[test]
fn a_manifest_verifies_only_under_its_key_and_as_signed() {
    let pair = |seed| Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap();
    let key = |pair: &Ed25519KeyPair| Key::new(pair.public_key().as_ref().try_into().unwrap());
    let (signer, other) = (pair(7), pair(8));
    let manifest = vun_data::json::Json(&release().to_data()).to_string();
    let signature = signer.sign(manifest.as_bytes());
    assert!(key(&signer).verify(manifest.as_bytes(), signature.as_ref()).is_ok());
    assert!(key(&other).verify(manifest.as_bytes(), signature.as_ref()).is_err());
    let changed = manifest.replace("0.4.2", "0.4.3");
    assert!(key(&signer).verify(changed.as_bytes(), signature.as_ref()).is_err());
    assert_eq!(Key::parse(&key(&signer).to_string()), Some(key(&signer)));
    assert_eq!(Key::parse("ed448:aaaa"), None);
}

#[test]
fn the_releases_key_reads_and_is_set() {
    assert_eq!(Key::parse(&Key::releases().to_string()), Some(Key::releases()));
    assert_ne!(
        Key::releases(),
        Key::new([0; 32]),
        "`key.pub` holds the placeholder: make the key (dev/commands.md)"
    );
}
