//! Place states (spec 05, section 2): reads need `init`, `&out` initializes, parts are tracked.

use crate::common::Fixture;
use overt_diag::errors;

#[test]
fn place_states_follow_writes_and_borrows() {
    let fixture = Fixture::new("places-states");
    let file = fixture.root.write(
        "places.ov",
        "\
fill := (h: &out u8) { h.* = 1; };
filled := () -> u8 { mut x: u8; fill(&out x); x };
parts := () -> u8 { mut buf: [4]u8; buf[1] = 2; buf[1] };
other := () -> u8 { mut buf: [4]u8; buf[1] = 2; buf[0] };
half := (flag: bool) -> u8 { mut y: u8; if flag { y = 1; } else { } y };
partly := (flag: bool, h: &out u8) { if flag { h.* = 1; } };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::PLACE_READ_UNINIT, errors::PLACE_AMBIGUOUS_JOIN, errors::PLACE_OUT_UNWRITTEN]);
}
