use super::*;

#[test]
fn lookup_finds_or_suggests() {
    let table = [("terminal", 0), ("short", 1)];
    assert_eq!(table.lookup("short", |(n, _)| n), Ok(&("short", 1)));
    assert_eq!(table.lookup("shrt", |(n, _)| n), Err(Some("short")));
    assert_eq!(table.lookup("json", |(n, _)| n), Err(None));
}

#[test]
fn suggests_only_close_names() {
    let names = ["build", "bench", "dump", "check"];
    assert_eq!(closest("biuld", names), Some("build"));
    assert_eq!(closest("dmp", names), Some("dump"));
    assert_eq!(closest("xyzzy", names), None);
    assert_eq!(distance("kitten", "sitting"), 3);
}

#[test]
fn a_name_is_never_suggested_to_itself() {
    assert_eq!(closest("shows", ["shows", "show"]), Some("show"));
    assert_eq!(closest("shows", ["shows"]), None);
}
