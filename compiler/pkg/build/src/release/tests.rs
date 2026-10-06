use vun_data::{FromData, ToData};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::{Machine, Name};
use vun_uri::Uri;

use ring::signature::{Ed25519KeyPair, KeyPair};

use super::{Build, Key, Piece, Profile, Release, Releases, Wanted};

fn release() -> Release {
    let machine = Machine { arch: Name("x86_64"), os: Name("linux") };
    let piece = Piece {
        name: "cranelift".into(),
        archive: Piece::archive_name("cranelift", machine),
        file: "libvun_component_cranelift.so".into(),
        hash: Hash::of(b"library"),
        profile: Profile::Default,
    };
    let build = Build {
        machine,
        archive: Build::archive_name(machine),
        hash: Hash::of(b"tree"),
        components: vec![piece],
    };
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
    let build = &read.builds[0];
    assert_eq!(build.piece("cranelift").map(|p| p.file.as_str()), Some("libvun_component_cranelift.so"));
    assert_eq!(build.in_profile(Profile::Minimal).count(), 0);
    assert_eq!(build.in_profile(Profile::Default).count(), 1);
    assert_eq!(build.in_profile(Profile::Full).count(), 1);
    assert!(build.piece("llvm").is_none());
    assert_eq!(Profile::named("full"), Some(Profile::Full));
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
    assert_eq!(
        path(releases.component(&release, &release.builds[0].components[0])),
        ["owner", "vun", "releases", "download", "0.4.2", "vun-cranelift-x86_64-linux.tar.gz"]
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

#[test]
fn install_sh_holds_the_releases_key() {
    use base64::Engine;
    const SHELL_DEFAULT: &str = "key=\"${VUN_INSTALL_KEY:-";
    // The header of an Ed25519 public key in DER, before its 32 bytes.
    const SPKI: [u8; 12] = [0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00];
    let script =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../install.sh")).unwrap();
    let held = script.lines().find_map(|l| l.strip_prefix(SHELL_DEFAULT)).and_then(|l| l.strip_suffix("}\""));
    let der =
        base64::engine::general_purpose::STANDARD.decode(held.expect("install.sh names its key")).unwrap();
    let (header, key) = der.split_at(SPKI.len());
    assert_eq!(header, SPKI);
    assert_eq!(Key::new(key.try_into().unwrap()), Key::releases());
}
