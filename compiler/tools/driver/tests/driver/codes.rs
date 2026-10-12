//! Enums whose variants name their tags (spec 05, section 6): a program reads them as it reads any
//! enum, through each backend, and C reads them as integers.

use vun_items::targets;

use crate::backend;
use crate::variadic::status;

/// `name = code` with negative codes, codes that follow the one before, `match`, a table of
/// constants and `==`: `20 + 10 + 40`.
const PURE: &str = "\
#[layout.tag(i32)]
Offset := enum { back = -2, here, ahead = 3 };

name := (o: Offset) -> u8 {
    match o {
        .back => 10,
        .here => 20,
        .ahead => 30,
    }
};

table: [3]Offset = [.ahead, .back, .here];

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    same := table[1] == Offset.back;
    std.os.Exit.of((name(table[2]) + name(table[1]) + if same { 40 } else { 0 }).to(Int(0..=125)))
};
";

#[test]
fn an_enum_that_names_its_tags_is_matched_stored_and_compared_by_each_backend() {
    assert_eq!(backend::status_across("enum-codes", PURE), Some(70));
}

/// C's `abs` takes the enum as the `int` its tag is: `2 + 3 + 1`, and the rest of `PURE`'s `70`.
const C_READS: &str = "\
#[layout.tag(i32)]
Offset := enum { back = -2, here, ahead = 3 };

#[link.symbol(\"abs\"), abi.c]
magnitude: unsafe (n: Offset) -> i32 = _;

table: [3]Offset = [.ahead, .back, .here];

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    a := unsafe { magnitude(table[1]) };
    b := unsafe { magnitude(table[0]) };
    c := unsafe { magnitude(.here) };
    std.os.Exit.of((a + b + c).to(Int(0..=125)))
};
";

#[test]
fn c_reads_the_tag_of_an_enum_as_its_integer() {
    if !cfg!(all(target_arch = "x86_64", target_os = "linux")) {
        return;
    }
    assert_eq!(status("enum-codes-c", C_READS, targets::linux_x86_64::gnu), Some(6));
}
