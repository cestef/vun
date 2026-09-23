use std::fs;

use flate2::Compression;
use flate2::write::GzEncoder;

use overt_fs::{Mode, Modes, Scratch};
use overt_hash::Snapshot;

use super::{Archive, Failure, Mirror};

/// A `.tar.gz` holding `files` under one top-level directory, as forges produce.
fn archive(files: &[(&str, &str, Mode)]) -> Vec<u8> {
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
    for (path, text, mode) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(text.len() as u64);
        header.set_mode(match mode {
            Mode::Regular => 0o644,
            Mode::Executable => 0o755,
        });
        header.set_cksum();
        tar.append_data(&mut header, format!("pkg-1.0/{path}"), text.as_bytes()).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn file_mirrors_unpack_without_the_top_level_directory() {
    let dir = Scratch::new("unpack");
    let path = dir.write(
        "pkg.tar.gz",
        archive(&[("a.ov", "a := 1;", Mode::Regular), ("sub/b.ov", "b := 2;", Mode::Regular)]),
    );
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
    let dir = Scratch::new("ovpkg");
    let src = dir.join("src");
    dir.write("src/build.ov", "package := 1;");
    dir.write("src/demo/a.ov", "a := 1;");
    let include = ["build.ov".to_owned(), "demo".to_owned()];
    let (bytes, snapshot) = Archive::pack(&src, &include).unwrap();
    assert_eq!(Archive::pack(&src, &include).unwrap().0, bytes, "identical trees, identical archives");
    let path = dir.write("demo.ovpkg", &bytes);
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

#[cfg(unix)]
#[test]
fn executable_files_stay_executable() {
    let dir = Scratch::new("modes");
    dir.write("src/build.ov", "package := 1;");
    dir.write("src/configure", "#!/bin/sh\n").set_mode(Mode::Executable).unwrap();
    let include = ["build.ov".to_owned(), "configure".to_owned()];
    let (bytes, snapshot) = Archive::pack(&dir.join("src"), &include).unwrap();
    let unpacked = Archive::unpack(&bytes, &dir.join("out")).unwrap();
    assert_eq!(unpacked, snapshot);
    assert_eq!(dir.join("out/configure").mode(), Mode::Executable);
    assert_eq!(dir.join("out/build.ov").mode(), Mode::Regular);
    assert_eq!(Snapshot::of(&dir.join("out"), &include).root(), snapshot.root());

    let path = dir.write("sdk.tar.gz", archive(&[("configure", "#!/bin/sh\n", Mode::Executable)]));
    Mirror::parse(&format!("file://{}", path.display())).unwrap().fetch(&dir.join("sdk")).unwrap();
    assert_eq!(dir.join("sdk/configure").mode(), Mode::Executable);
}
