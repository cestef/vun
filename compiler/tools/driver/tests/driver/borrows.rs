//! Borrow rules (spec 05, sections 4 and 8): exclusive loans live until their last use, and a
//! borrow of a function's own storage cannot be returned.

use crate::common::Fixture;
use overt_diag::errors;

#[test]
fn exclusive_loans_conflict_only_while_used() {
    let fixture = Fixture::new("borrows-conflict");
    let file = fixture.root.write(
        "conflict.ov",
        "\
Point := struct { x: u8, y: u8 };
shared := (xs: &mut [4]u8) -> u16 { a := &xs[0]; b := &xs[1]; a.* + b.* };
finished := (xs: &mut [4]u8) { first := &mut xs[0]; first.* = 1; xs[1] = 2; };
fields := (p: &mut Point) { x := &mut p.x; p.y = 2; x.* = 1; };
reborrowed := (xs: &mut [4]u8) { first := &mut xs[0]; again := &mut first.*; xs[1] = 2; again.* = 1; };
read := (p: &mut Point) -> u8 { x := &mut p.x; y := p.x; x.* = 1; y };
written := (p: &mut Point) { x := &p.x; p.x = 2; y := x.*; };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::BORROW_CONFLICT, errors::BORROW_CONFLICT, errors::BORROW_CONFLICT]);
}

#[test]
fn borrows_of_own_storage_do_not_escape() {
    let fixture = Fixture::new("borrows-escape");
    let file = fixture.root.write(
        "escape.ov",
        "\
first := (xs: &[4]u8) -> &u8 { &xs[0] };
pick := (a: &u8, b: &u8) -> &u8 { a };
lent := (a: &u8) -> &u8 { x: u8 = 1; pick(a, &x) };
through := (xs: &[4]u8) -> &u8 { first(xs) };
param := (x: u8) -> &u8 { &x };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::BORROW_ESCAPE, errors::BORROW_ESCAPE]);
}
