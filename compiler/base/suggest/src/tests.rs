use super::*;

#[test]
fn suggests_only_close_names() {
    let names = ["build", "bench", "dump", "check"];
    assert_eq!(closest("biuld", names), Some("build"));
    assert_eq!(closest("dmp", names), Some("dump"));
    assert_eq!(closest("xyzzy", names), None);
    assert_eq!(distance("kitten", "sitting"), 3);
}
