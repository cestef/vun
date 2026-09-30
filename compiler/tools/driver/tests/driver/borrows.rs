//! Borrow rules (spec 05, sections 4 and 8): exclusive loans live until their last use, and a
//! borrow of a function's own storage cannot be returned.

use crate::common::Fixture;
use vun_diag::errors;
use vun_span::SourceMap;

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
written := (p: &mut Point) { x := &p.x; p.x = 2; _: u8 = x.*; };
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

#[test]
fn borrows_do_not_outlive_the_scope_of_what_they_borrow() {
    let fixture = Fixture::new("borrows-scopes");
    let file = fixture.root.write(
        "scopes.vun",
        "\
zero: u8 = 0;
outlived := () -> u8 { mut r: &u8 = &zero; { x: u8 = 1; r = &x; } r.* };
within := () -> u8 { x: u8 = 1; mut r: &u8 = &zero; { r = &x; } r.* };
unused := () { mut r: &u8 = &zero; { x: u8 = 1; r = &x; } };
",
    );
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(ids, [errors::BORROW_ESCAPE], "{found:?}");
}

#[test]
fn borrows_of_what_lasts_are_static() {
    let fixture = Fixture::new("borrows-static");
    let file = fixture.root.write(
        "lasting.vun",
        "\
zero: u8 = 0;
mut count: u8 = 0;
Empty := struct { items: &static [u8] };
literal := () -> Empty { .{ .items = &[] } };
binding := () -> &static u8 { &zero };
local := () -> &static u8 { x: u8 = 1; &x };
counted := () -> &static u8 { unsafe { &count } };
",
    );
    // A literal and an immutable top-level binding live as long as the program; a local does
    // not, and a `mut` binding may change while borrowed.
    assert_eq!(fixture.check(&file), [errors::TYPES_MISMATCH, errors::TYPES_MISMATCH]);
}

#[test]
fn a_conflict_names_where_the_borrow_is_used_later() {
    let fixture = Fixture::new("borrows-later");
    let file = fixture.root.write(
        "later.vun",
        "\
bump := (x: &mut u8) { x.* = 1; };
conflict := () -> u8 {
    mut n: u8 = 1;
    r := &n;
    bump(&mut n);
    r.*
};
",
    );
    let mut session = fixture.session();
    let file = session.load(&file).unwrap();
    session.check(file);
    let reports = session.diag.sorted(session.origins);
    let [conflict] = reports.as_slice() else { panic!("one report: {reports:?}") };
    assert_eq!(conflict.id, errors::BORROW_CONFLICT);
    let line = |origin| session.sources.locate(session.origins.source_span(origin)).at.line;
    let lines: Vec<_> = conflict.labels.iter().map(|l| line(l.origin)).collect();
    // Where the borrow is taken, then where it is used after the conflicting access.
    assert_eq!(lines, [4, 6]);
}

#[test]
fn parts_of_immutable_top_level_bindings_are_borrowed_for_the_program() {
    let fixture = Fixture::new("borrows-static");
    let file = fixture.root.write(
        "statics.vun",
        "\
table: [4]u8 = [1, 2, 3, 4];
Config := struct { limit: u8, name: [3]u8 };
config: Config = .{ .limit = 9, .name = [5, 6, 7] };
mut counter: u8 = 0;
item := () -> &static u8 { t := &table[2]; t };
field := () -> &static u8 { l := &config.limit; l };
nested := () -> &static u8 { n := &config.name[2]; n };
local := () -> &static u8 { xs: [2]u8 = [1, 2]; p := &xs[0]; p };
changing := () -> &static u8 { c := unsafe { &counter }; c };
",
    );
    // A local's storage and a `mut` static's are not the program's to lend.
    assert_eq!(fixture.check(&file), [errors::TYPES_MISMATCH, errors::TYPES_MISMATCH]);
}
