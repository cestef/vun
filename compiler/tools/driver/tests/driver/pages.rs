//! `std.os.pages`: changing what a range of the process's own pages allow, and finding the
//! range an address is mapped within (spec 08, section 2).

use vun_items::targets;

use crate::backend::status_on;

/// A program narrowing a global's page to read-only, confirming it through a query, then
/// widening it back: 9 where every step agrees with what it changed.
const ROUND_TRIP: &str = "\
start := std.os.start(main);

mut flag: u8 = 0;
page := 4096;

main := (sys: &mut std.os.System) -> std.os.Exit {
    at := unsafe std.ptr.to(u8, &flag).address();
    base := (at - at % page).wrap(usize);
    before := match std.os.pages.query(at) {
        .ok(r) => r,
        .err(_) => return .code(1),
    };
    if !before.access.read or !before.access.write { return .code(2); }
    match unsafe std.os.pages.protect(base, page, std.os.pages.read_only) {
        .ok(_) => {},
        .err(_) => return .code(3),
    }
    narrowed := match std.os.pages.query(at) {
        .ok(r) => r,
        .err(_) => return .code(4),
    };
    if !narrowed.access.read or narrowed.access.write { return .code(5); }
    match unsafe std.os.pages.protect(base, page, std.os.pages.read_write) {
        .ok(_) => {},
        .err(_) => return .code(6),
    }
    widened := match std.os.pages.query(at) {
        .ok(r) => r,
        .err(_) => return .code(7),
    };
    if !widened.access.read or !widened.access.write { return .code(8); }
    .code(9)
};
";

#[test]
fn a_page_is_narrowed_and_widened_as_a_query_confirms() {
    let cross = [
        targets::linux_arm64::kernel,
        targets::linux_arm::kernel,
        targets::windows_x86_64::msvc,
        targets::windows_x86::msvc,
    ];
    assert_eq!(status_on("pages-round-trip", ROUND_TRIP, &cross), Some(9));
}
