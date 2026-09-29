use std::path::Path;

use crate::{NotFile, Uri};

#[cfg(unix)]
#[test]
fn paths_and_file_uris_convert_both_ways() {
    for (path, uri) in [
        ("/home/a b/ü.vun", "file:///home/a%20b/%C3%BC.vun"),
        ("/x/%?#[]", "file:///x/%25%3F%23%5B%5D"),
        ("/", "file:///"),
    ] {
        let written = Uri::from_file_path(Path::new(path)).unwrap();
        assert_eq!(written.as_str(), uri);
        written.verify().unwrap();
        assert_eq!(written.to_file_path().unwrap(), Path::new(path));
    }
    let local: Uri = "file://LocalHost/etc/x".parse().unwrap();
    assert_eq!(local.to_file_path().unwrap(), Path::new("/etc/x"));
    let remote: Uri = "file://server/etc/x".parse().unwrap();
    assert_eq!(remote.to_file_path(), Err(NotFile::Host));
    let web: Uri = "https://example.org/x".parse().unwrap();
    assert_eq!(web.to_file_path(), Err(NotFile::Scheme));
    assert_eq!(Uri::from_file_path(Path::new("rel/x")), Err(NotFile::Relative));
}

#[cfg(unix)]
#[test]
fn bytes_that_are_not_utf8_survive() {
    use std::os::unix::ffi::OsStrExt;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"/a\xFFb"));
    let uri = Uri::from_file_path(path).unwrap();
    assert_eq!(uri.as_str(), "file:///a%FFb");
    assert_eq!(uri.to_file_path().unwrap(), path);
}
