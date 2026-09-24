//! Layout (spec 05, section 6): natural alignment and padding, niches, layout attributes.

use crate::common::Fixture;
use vun_diag::errors;

#[test]
fn sizes_follow_the_layout_rules() {
    let fixture = Fixture::new("layout-sizes");
    let file = fixture.root.write(
        "sizes.vun",
        "\
Config := struct { baud: u32, parity: bool };
u3 := unsigned(3);
#[layout.bits(u32)]
Ctrl := struct { enable: bool, mode: u3, _: unsigned(28) };
Mixed := enum { a, b(u32), c(u8, u8) };
#[layout.align(64)]
Line := struct { bytes: [4]u8 };
config: Int(0..=std.meta.size(Config)) = 9;
pointer: Int(0..=std.meta.size(?*u8)) = 9;
niche: Int(0..=std.meta.size(?Int(0..200))) = 2;
tagged: Int(0..=std.meta.size(?u8)) = 3;
slice: Int(0..=std.meta.size(&[u8])) = 17;
ctrl: Int(0..=std.meta.size(Ctrl)) = 5;
mixed: Int(0..=std.meta.size(Mixed)) = 9;
line: Int(0..=std.meta.size(Line)) = 65;
#[layout.bits(u8)]
Short := struct { enable: bool, mode: u3 };
",
    );
    // Each size is one less than the value it bounds, so each value does not fit.
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
    let mut expected = vec![errors::INT_DOES_NOT_FIT; 8];
    expected.push(errors::LAYOUT_BITS_WIDTH);
    assert_eq!(ids, expected);
    assert_eq!(found[8].1, "the fields take 4 bits, not the 8 of `Int(0..=255)`");
}

#[test]
fn c_structs_hold_only_what_c_reads_the_same_way() {
    let fixture = Fixture::new("layout-c");
    let file = fixture.root.write(
        "c.vun",
        "\
#[layout.c]
Inner := struct { x: i32 };
Plain := struct { x: i32 };
#[layout.c]
Good := struct {
    a: u8, b: i64, c: bool, d: f64, e: *u8, f: ?*u8, g: [4]u16, h: Inner,
    cb: ?#[abi.c] (x: i32) -> i32,
};
good: Int(0..=std.meta.size(Good)) = 73;
#[layout.c]
Bad := struct {
    a: Int(0..200), b: u128, c: &u8, d: Plain, e: (x: i32) -> i32, f: ?u8, g: [2]Int(1..3),
};
#[layout.c]
Empty := struct {};
",
    );
    // `Good` takes 72 bytes as C lays it out: `a` and `c` padded to 8, `g` 8, `h` 4 padded to 8,
    // then `cb` at 64.
    let found = fixture.reported(&file);
    let ids: Vec<_> = found.iter().map(|(id, _)| id.clone()).collect();
    let mut expected = vec![errors::INT_DOES_NOT_FIT];
    expected.extend([errors::LAYOUT_NOT_C; 8]);
    assert_eq!(ids, expected, "{found:?}");
}
