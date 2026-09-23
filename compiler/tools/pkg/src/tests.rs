use std::io::Read;

use overt_fs::{Mode, Modes, Scratch};
use overt_hash::Snapshot;

use super::{Archive, FORMATS, Failure, Mirror, Scheme, Zstd};

/// A package tree with a nested file, an executable one and a path past the ustar name field.
fn tree(dir: &Scratch) -> Vec<String> {
    dir.write("src/build.ov", "package := 1;");
    dir.write("src/demo/a.ov", "a := 1;");
    dir.write("src/configure", "#!/bin/sh\n").set_mode(Mode::Executable).unwrap();
    dir.write(format!("src/demo/{}/deep.ov", "d".repeat(120)), "deep := 1;");
    ["build.ov", "configure", "demo"].map(str::to_owned).to_vec()
}

#[test]
fn every_format_packs_reproducibly_and_unpacks_to_the_same_tree() {
    let dir = Scratch::new("formats");
    let include = tree(&dir);
    for format in FORMATS {
        let (bytes, snapshot) = format.pack(&dir.join("src"), &include, "demo-1.0.0").unwrap();
        let (again, _) = format.pack(&dir.join("src"), &include, "demo-1.0.0").unwrap();
        assert_eq!(bytes, again, "{}: identical trees, identical archives", format.suffix());
        let path = dir.write(["demo", format.suffix()].concat(), &bytes);
        let out = dir.join(format.suffix());
        Mirror::parse(&Scheme::file_url(&path)).unwrap().fetch(&out).unwrap();
        assert_eq!(Snapshot::of(&out, &include), snapshot, "{}", format.suffix());
        #[cfg(unix)]
        assert_eq!(out.join("configure").mode(), Mode::Executable, "{}", format.suffix());
        assert!(!dir.join(format!("{}.unpacking", format.suffix())).exists());
    }
}

#[test]
fn unknown_schemes_and_formats_are_named() {
    assert!(matches!(Mirror::parse("ftp://x/a.tar.gz"), Err(Failure::Scheme)));
    assert!(matches!(Mirror::parse("https://x/a.rar"), Err(Failure::Format)));
    assert!(Mirror::parse("https://x/a.tar.gz?download=1").is_ok());
}

#[test]
fn package_archives_verify_every_file() {
    let dir = Scratch::new("ovpkg");
    let include = tree(&dir);
    let (bytes, _) = Archive::FORMAT.pack(&dir.join("src"), &include, "demo-1.0.0").unwrap();
    // A file whose bytes differ from the index is refused.
    let mut raw = Vec::new();
    Zstd::reader(&bytes).unwrap().read_to_end(&mut raw).unwrap();
    let at = raw.windows(7).position(|w| w == b"a := 1;").unwrap();
    raw[at + 5] = b'2';
    let err = Archive::unpack(&Zstd::compress(&raw), &dir.join("bad")).unwrap_err();
    assert!(err.to_string().contains("demo/a.ov"), "{err}");
}
