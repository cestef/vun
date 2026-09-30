use vun_data::{FromData, ToData};
use vun_hash::Hash;
use vun_span::Version;
use vun_target::{Machine, Name};
use vun_uri::Uri;

use super::{Build, Release, Releases, Wanted};

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
    let releases = Releases::at(Uri::parse("https://forge.example/owner/vun").unwrap());
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
