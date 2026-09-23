use std::fs;
use std::path::PathBuf;

use flate2::Compression;
use flate2::write::GzEncoder;

use overt_hash::Snapshot;

use super::{Archive, Failure, Mirror};

/// A fresh directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("overt-pkg-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A `.tar.gz` holding `files` under one top-level directory, as forges produce.
fn archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (path, text) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(text.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, format!("pkg-1.0/{path}"), text.as_bytes()).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn file_mirrors_unpack_without_the_top_level_directory() {
    let dir = scratch("unpack");
    let path = dir.join("pkg.tar.gz");
    fs::write(&path, archive(&[("a.ov", "a := 1;"), ("sub/b.ov", "b := 2;")])).unwrap();
    let url = format!("file://{}", path.display());
    Mirror::parse(&url).unwrap().fetch(&dir.join("out")).unwrap();
    assert_eq!(fs::read_to_string(dir.join("out/sub/b.ov")).unwrap(), "b := 2;");
    assert!(!dir.join("out.unpacking").exists());
}

#[test]
fn unknown_schemes_and_formats_are_named() {
    assert!(matches!(Mirror::parse("ftp://x/a.tar.gz"), Err(Failure::Scheme)));
    assert!(matches!(Mirror::parse("https://x/a.rar"), Err(Failure::Format)));
    assert!(Mirror::parse("https://x/a.tar.gz?download=1").is_ok());
}

#[test]
fn package_archives_are_reproducible_and_verified() {
    let dir = scratch("ovpkg");
    let src = dir.join("src");
    fs::create_dir_all(src.join("demo")).unwrap();
    fs::write(src.join("build.ov"), "package := 1;").unwrap();
    fs::write(src.join("demo/a.ov"), "a := 1;").unwrap();
    let include = ["build.ov".to_owned(), "demo".to_owned()];
    let (bytes, snapshot) = Archive::pack(&src, &include).unwrap();
    assert_eq!(Archive::pack(&src, &include).unwrap().0, bytes, "identical trees, identical archives");
    let path = dir.join("demo.ovpkg");
    fs::write(&path, &bytes).unwrap();
    Mirror::parse(&format!("file://{}", path.display())).unwrap().fetch(&dir.join("out")).unwrap();
    assert_eq!(Snapshot::of(&dir.join("out"), &include), snapshot);
    // A file whose bytes differ from the index is refused.
    let mut raw = zstd::decode_all(bytes.as_slice()).unwrap();
    let at = raw.windows(7).position(|w| w == b"a := 1;").unwrap();
    raw[at + 5] = b'2';
    let tampered = zstd::encode_all(raw.as_slice(), 1).unwrap();
    let err = Archive::unpack(&tampered, &dir.join("bad")).unwrap_err();
    assert!(err.to_string().contains("demo/a.ov"), "{err}");
}
