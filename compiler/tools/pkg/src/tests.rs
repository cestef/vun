use std::fs;
use std::io::{Read, Write};

use flate2::Compression;
use flate2::write::GzEncoder;

use overt_fs::{Mode, Modes, Scratch};
use overt_hash::Snapshot;

use super::{Archive, Failure, Mirror, Scheme, Tar, Zstd};

/// One tar entry: a ustar header of `kind` for `name`, then `data` padded to a block.
fn entry(name: &str, kind: u8, mode: u32, data: &[u8]) -> Vec<u8> {
    let mut header = [0_u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    header[100..107].copy_from_slice(format!("{mode:07o}").as_bytes());
    header[124..135].copy_from_slice(format!("{:011o}", data.len()).as_bytes());
    header[156] = kind;
    header[257..263].copy_from_slice(b"ustar\0");
    header[148..156].fill(b' ');
    let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
    header[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
    let mut out = header.to_vec();
    out.extend_from_slice(data);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

/// A tar archive of `entries`, ended by two zero blocks.
fn tar(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut out = entries.concat();
    out.extend_from_slice(&[0; 1024]);
    out
}

/// A `.tar.gz` holding `files` under one top-level directory, as forges produce.
fn archive(files: &[(&str, &str, Mode)]) -> Vec<u8> {
    let entries: Vec<_> = files
        .iter()
        .map(|(path, text, mode)| {
            let mode = match mode {
                Mode::Regular => 0o644,
                Mode::Executable => 0o755,
            };
            entry(&format!("pkg-1.0/{path}"), b'0', mode, text.as_bytes())
        })
        .collect();
    let mut gz = GzEncoder::new(Vec::new(), Compression::fast());
    gz.write_all(&tar(&entries)).unwrap();
    gz.finish().unwrap()
}

#[test]
fn tar_paths_come_from_gnu_and_pax_records() {
    let dir = Scratch::new("tarpaths");
    let long = format!("{}/deep.ov", "d".repeat(120));
    let pax = "21 path=pax/named.ov\n";
    let bytes = tar(&[
        entry("././@LongLink", b'L', 0o644, long.as_bytes()),
        entry("ignored", b'0', 0o644, b"long"),
        entry("././@LongLink", b'L', 0o644, b"a-long-directory/"),
        entry("dir/", b'5', 0o755, b""),
        entry("PaxHeader", b'x', 0o644, pax.as_bytes()),
        entry("short", b'0', 0o644, b"pax"),
        entry("./plain.ov", b'0', 0o644, b"plain"),
    ]);
    Tar::unpack(bytes.as_slice(), &dir.join("out")).unwrap();
    assert_eq!(fs::read_to_string(dir.join("out").join(&long)).unwrap(), "long");
    assert_eq!(fs::read_to_string(dir.join("out/pax/named.ov")).unwrap(), "pax");
    assert_eq!(fs::read_to_string(dir.join("out/plain.ov")).unwrap(), "plain");
}

#[test]
fn tar_links_and_escaping_paths_are_refused() {
    let dir = Scratch::new("tarbad");
    for bad in [entry("link", b'2', 0o777, b""), entry("../escape.ov", b'0', 0o644, b"x")] {
        let err = Tar::unpack(tar(&[bad]).as_slice(), &dir.join("out")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "{err}");
    }
    assert!(!dir.join("escape.ov").exists());
}

#[test]
fn file_mirrors_unpack_without_the_top_level_directory() {
    let dir = Scratch::new("unpack");
    let path = dir.write(
        "pkg.tar.gz",
        archive(&[("a.ov", "a := 1;", Mode::Regular), ("sub/b.ov", "b := 2;", Mode::Regular)]),
    );
    let url = Scheme::file_url(&path);
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
    Mirror::parse(&Scheme::file_url(&path)).unwrap().fetch(&dir.join("out")).unwrap();
    assert_eq!(Snapshot::of(&dir.join("out"), &include), snapshot);
    // A file whose bytes differ from the index is refused.
    let mut raw = Vec::new();
    Zstd::reader(&bytes).unwrap().read_to_end(&mut raw).unwrap();
    let at = raw.windows(7).position(|w| w == b"a := 1;").unwrap();
    raw[at + 5] = b'2';
    let tampered = Zstd::compress(&raw);
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
    Mirror::parse(&Scheme::file_url(&path)).unwrap().fetch(&dir.join("sdk")).unwrap();
    assert_eq!(dir.join("sdk/configure").mode(), Mode::Executable);
}
