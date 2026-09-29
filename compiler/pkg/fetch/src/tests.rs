use vun_fs::{Mode, Modes, Scratch};
use vun_hash::Snapshot;

use vun_uri::Uri;

use super::{FORMATS, Failure, Mirror, Scheme, Schemes, Zip};
use crate::unpack::Refused;

/// A package tree with a nested file, an executable one and a path past the ustar name field.
fn tree(dir: &Scratch) -> Vec<String> {
    dir.write("src/build.vun", "package := 1;");
    dir.write("src/demo/a.vun", "a := 1;");
    dir.write("src/configure", "#!/bin/sh\n").set_mode(Mode::Executable).unwrap();
    dir.write(format!("src/demo/{}/deep.vun", "d".repeat(120)), "deep := 1;");
    ["build.vun", "configure", "demo"].map(str::to_owned).to_vec()
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
        Mirror::of(&Uri::from_file_path(&path).unwrap(), &Schemes::default()).unwrap().fetch(&out).unwrap();
        assert_eq!(Snapshot::of(&out, &include).unwrap(), snapshot, "{}", format.suffix());
        #[cfg(unix)]
        assert_eq!(out.join("configure").mode(), Mode::Executable, "{}", format.suffix());
        assert!(!Mirror::unpacking(&out).exists());
    }
}

#[test]
fn unknown_schemes_and_formats_are_named() {
    /// A network scheme as a program installs one; never read here.
    const NETWORK: &[Scheme] = &[Scheme { name: "https", read: |_| Ok(Vec::new()) }];
    let mut schemes = Schemes::default();
    assert!(matches!(Mirror::of(&"https://x/a.tar.gz".parse().unwrap(), &schemes), Err(Failure::Scheme(_))));
    schemes.install(NETWORK);
    let Err(unknown) = Mirror::of(&"ftp://x/a.tar.gz".parse().unwrap(), &schemes) else {
        panic!("ftp is no scheme")
    };
    assert_eq!(unknown.to_string(), "no supported scheme; supported: file, https");
    assert!(matches!(Mirror::of(&"https://x/a.rar".parse().unwrap(), &schemes), Err(Failure::Format)));
    assert!(Mirror::of(&"https://x/a.tar.gz?download=1".parse().unwrap(), &schemes).is_ok());
}

#[test]
fn a_zip_holding_a_link_is_refused() {
    use std::io::{Cursor, Write};
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("a.vun", SimpleFileOptions::default()).unwrap();
    zip.write_all(b"a := 1;").unwrap();
    zip.add_symlink("home", "/home", SimpleFileOptions::default()).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    let dir = Scratch::new("zip-link");
    let error = Zip::unpack(&bytes, &dir.join("out")).unwrap_err();
    assert!(matches!(error.get_ref().and_then(|e| e.downcast_ref()), Some(Refused::Entry(_))));
}

#[test]
fn a_local_path_keeps_what_a_url_would_read_as_a_query() {
    let schemes = Schemes::default();
    let path = std::path::Path::new("/tmp/v#1/a.tar.gz");
    assert!(Mirror::of(&Uri::from_file_path(path).unwrap(), &schemes).is_ok());
}
