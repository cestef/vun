use std::io;

use overt_fs::{Mode, Scratch};

use super::{Header, Kind, Tar};

/// An archive of raw entries, each a kind, a name and data, ended as tar archives are.
fn archive(entries: &[(Kind, &str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for &(kind, name, data) in entries {
        Tar::entry(&mut out, kind, name, Mode::Regular, data);
    }
    Tar::end(&mut out);
    out
}

#[test]
fn paths_come_from_gnu_and_pax_records() {
    let dir = Scratch::new("tarpaths");
    let long = format!("{}/deep.ov", "d".repeat(120));
    let pax = Tar::pax_record("path", "pax/named.ov");
    let bytes = archive(&[
        (Kind::LongName, "././@LongLink", long.as_bytes()),
        (Kind::File, "ignored", b"long"),
        (Kind::LongName, "././@LongLink", b"a-long-directory/"),
        (Kind::Directory, "dir/", b""),
        (Kind::Extended, "PaxHeader", pax.as_bytes()),
        (Kind::File, "short", b"pax"),
        (Kind::File, "./plain.ov", b"plain"),
    ]);
    Tar::unpack(bytes.as_slice(), &dir.join("out")).unwrap();
    let read = |path: &str| std::fs::read_to_string(dir.join("out").join(path)).unwrap();
    assert_eq!(
        (read(&long), read("pax/named.ov"), read("plain.ov")),
        ("long".into(), "pax".into(), "plain".into())
    );
    assert_eq!(pax, "21 path=pax/named.ov\n", "the length counts its own digits");
}

#[test]
fn links_escaping_paths_and_bad_checksums_are_refused() {
    let dir = Scratch::new("tarbad");
    let mut corrupt = archive(&[(Kind::File, "a.ov", b"a")]);
    corrupt[0] = b'b';
    let bad = [
        archive(&[(Kind::Other(b'2'), "link", b"")]),
        archive(&[(Kind::File, "../escape.ov", b"x")]),
        corrupt,
    ];
    for bytes in bad {
        let err = Tar::unpack(bytes.as_slice(), &dir.join("out")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData, "{err}");
    }
    assert!(!dir.join("escape.ov").exists());
    assert_eq!(Header::new(Kind::File, "x", Mode::Executable, 0).mode().unwrap(), Mode::Executable);
}
