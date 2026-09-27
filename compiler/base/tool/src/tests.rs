use std::ffi::OsStr;
use std::path::PathBuf;

use super::Arg;

#[test]
fn paths_never_pass_as_options() {
    let os = |path: &str| Arg::Path(PathBuf::from(path)).os().into_owned();
    assert_eq!(os("-o.o"), OsStr::new("./-o.o"));
    assert_eq!(os("a/-b.o"), OsStr::new("a/-b.o"));
    assert_eq!(os("/tmp/-b.o"), OsStr::new("/tmp/-b.o"));
    assert_eq!(Arg::from("-o").os(), OsStr::new("-o"), "text is passed as written");
}
