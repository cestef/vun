//! Borrow rules (spec 05, sections 4 and 8): exclusive loans live until their last use, and a
//! borrow of a function's own storage cannot be returned.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn exclusive_loans_conflict_only_while_used() {
    let fixture = Fixture::new("borrows-conflict");
    let file = fixture.root.write(
        "conflict.vun",
        "\
Point := struct { x: u8, y: u8 };
shared := (xs: &mut [4]u8) -> u16 { a := &xs[0]; b := &xs[1]; a.* + b.* };
finished := (xs: &mut [4]u8) { first := &mut xs[0]; first.* = 1; xs[1] = 2; };
fields := (p: &mut Point) { x := &mut p.x; p.y = 2; x.* = 1; };
reborrowed := (xs: &mut [4]u8) { first := &mut xs[0]; again := &mut first.*; xs[1] = 2; again.* = 1; };
read := (p: &mut Point) -> u8 { x := &mut p.x; y := p.x; x.* = 1; y };
written := (p: &mut Point) { x := &p.x; p.x = 2; y := x.*; };
Buf := struct { n: u8, push := (self: &mut Buf, x: u8) { self.n = x; }; len := (self: &Buf) -> u8 { self.n }; };
grow := (b: &mut Buf) { b.push(b.len()); };
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::BORROW_CONFLICT, errors::BORROW_CONFLICT, errors::BORROW_CONFLICT]);
}

#[test]
fn borrows_of_own_storage_do_not_escape() {
    let fixture = Fixture::new("borrows-escape");
    let file = fixture.root.write(
        "escape.vun",
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

#[test]
fn free_types_hold_no_borrows_and_owners_deref() {
    let fixture = Fixture::new("borrows-free");
    let file = fixture.root.write(
        "free.vun",
        "\
Node := struct { value: u8, next: ?Own(Node) };
Own := ($T: type) -> type { #[kind.linear] #[kind.free] struct { #[kind.owns] ptr: *T } };
Parser := struct { input: &[u8] };
Boxed := #[kind.free] struct { parser: Parser };
boxed := (p: Parser) -> Boxed { .{ .parser = p } };
owned := (p: *Parser) -> Own(Parser) { .{ .ptr = p } };
value := (o: &Own(Node)) -> u8 { o.value };
walk := (head: &?Own(Node)) -> u8 {
    mut cur := head;
    mut last: u8 = 0;
    for .some(node) = cur { last = node.value; cur = &node.next; }
    last
};
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::BORROW_IN_FREE_VALUE, errors::BORROW_IN_FREE_VALUE]);
}

#[test]
fn temp_borrows_are_lent_for_the_call() {
    let fixture = Fixture::new("borrows-temp");
    let file = fixture.root.write(
        "temp.vun",
        "\
find := (hay: &[4]u8, needle: &temp [4]u8) -> &[4]u8 { hay };
leak := (hay: &[4]u8, needle: &temp [4]u8) -> &[4]u8 { needle };
bump := (x: &temp mut u8) { x.* = 1; };
untied := () -> u8 {
    a: [4]u8 = [1, 2, 3, 4];
    mut b: [4]u8 = [4, 4, 4, 4];
    r := find(&a, &b);
    b[0] = 5;
    bump(&mut b[1]);
    r[0]
};
tied := () -> u8 {
    a: [4]u8 = [1, 2, 3, 4];
    mut c: [4]u8 = [4, 4, 4, 4];
    s := find(&c, &a);
    c[0] = 1;
    s[0]
};
",
    );
    let ids: Vec<_> = fixture.reported(&file).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [errors::BORROW_ESCAPE, errors::BORROW_CONFLICT]);
}
