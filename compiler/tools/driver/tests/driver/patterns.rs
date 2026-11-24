//! Literal and range patterns are comparisons with the value matched (spec 01, section 6;
//! spec 02, section 4): an integer in a pattern is an operand, as in an expression, and need not
//! fit the matched type, which in an instantiation is its `$` argument's alone.

use crate::backend::status;
use crate::support::Fixture;

/// Arms over a `$` parameter, an instantiation typing it as its value alone, and over a `u8`,
/// with bounds outside the type.
const ARMS: &str = "\
kind := ($n: u8) -> u8 {
    match n {
        0..=9 => 1,
        5 => 3,
        10..=300 => 2,
    }
};
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    x: u8 = 200;
    r: u8 = match x {
        0..10 => 10,
        300 => 30,
        10.. => 20,
    };
    std.os.Exit.of((kind(3) + kind(50) + r).to(Int(0..=125)))
};
";

#[test]
fn integers_in_patterns_are_operands_that_need_not_fit_the_matched_type() {
    let fixture = Fixture::new("patterns-operands");
    let file = fixture.root.write("main.vun", ARMS);
    assert_eq!(fixture.check(&file), []);
    assert_eq!(status("patterns-operands-run", ARMS), Some(23));
}
