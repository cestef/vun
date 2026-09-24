use vun_fs::{Mode, Modes, Scratch};
use vun_hash::Snapshot;

use super::{FORMATS, Failure, Mirror, Scheme, Schemes};

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
        Mirror::parse(&Scheme::file_url(&path), &Schemes::default()).unwrap().fetch(&out).unwrap();
        assert_eq!(Snapshot::of(&out, &include), snapshot, "{}", format.suffix());
        #[cfg(unix)]
        assert_eq!(out.join("configure").mode(), Mode::Executable, "{}", format.suffix());
        assert!(!dir.join(format!("{}.unpacking", format.suffix())).exists());
    }
}

#[test]
fn unknown_schemes_and_formats_are_named() {
    /// A network scheme as a program installs one; never read here.
    const NETWORK: &[Scheme] = &[Scheme { prefix: "https://", read: |_, _| Ok(Vec::new()) }];
    let mut schemes = Schemes::default();
    assert!(matches!(Mirror::parse("https://x/a.tar.gz", &schemes), Err(Failure::Scheme(_))));
    schemes.install(NETWORK);
    let Err(unknown) = Mirror::parse("ftp://x/a.tar.gz", &schemes) else { panic!("ftp is no scheme") };
    assert_eq!(unknown.to_string(), "no supported scheme; supported: file://, https://");
    assert!(matches!(Mirror::parse("https://x/a.rar", &schemes), Err(Failure::Format)));
    assert!(Mirror::parse("https://x/a.tar.gz?download=1", &schemes).is_ok());
}
