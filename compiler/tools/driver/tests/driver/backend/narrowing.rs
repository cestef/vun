//! Integers brought into other ranges and shifted, run through every backend: what the programs
//! made by `vun debug smith` found wrong.

use super::{BACKENDS, run};

/// What `source` prints through each backend, which must be alike.
fn printed(name: &str, source: &str) -> Vec<u8> {
    let mut outputs = BACKENDS.map(|backend| {
        let output = run(name, source, backend);
        assert!(output.status.success(), "{backend}: {}", String::from_utf8_lossy(&output.stderr));
        output.stdout
    });
    let first = std::mem::take(&mut outputs[0]);
    assert!(outputs[1..].iter().all(|o| *o == first), "the backends part");
    first
}

#[test]
fn a_range_apart_from_the_source_saturates_to_its_nearest_bound() {
    let source = "\
start := std.os.start(main);
below := (a: u16) -> Int(-9..=-5) { a.sat(Int(-9..=-5)) };
above := (a: i8) -> Int(300..=400) { a.sat(Int(300..=400)) };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.println(&mut out, \"{}\", below(7));
    std.fmt.println(&mut out, \"{}\", above(-3));
    .ok
};
";
    assert_eq!(printed("sat-apart", source), b"-5\n300\n");
}

#[test]
fn a_size_past_the_storage_wraps_in_128_bits() {
    let source = "\
start := std.os.start(main);
wide := (a: i128) -> Int(-984394..=1205532464550) { a.wrap(Int(-984394..=1205532464550)) };
huge := (a: u128) -> Int(3..=1000000000000000000000) { a.wrap(Int(3..=1000000000000000000000)) };
long := (a: Int(-5..=300)) -> Int(0..=340282366920938463463374607431768211454) {
    a.wrap(Int(0..=340282366920938463463374607431768211454))
};
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.println(&mut out, \"{}\", wide(-170141183460469231731687303715884105728));
    std.fmt.println(&mut out, \"{}\", huge(340282366920938463463374607431768211455));
    std.fmt.println(&mut out, \"{}\", long((-5).wrap(Int(-5..=300))));
    .ok
};
";
    let text = String::from_utf8(printed("wrap-wide", source)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[2], "340282366920938463463374607431768211450");
}

#[test]
fn a_shift_left_keeps_the_bits_of_its_type() {
    let source = "\
start := std.os.start(main);
shifted := (a: Int(0..=7), n: Int(0..=2)) -> Int(0..=7) { a << n };
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.println(&mut out, \"{}\", shifted((7).wrap(Int(0..=7)), (2).wrap(Int(0..=2))));
    .ok
};
";
    assert_eq!(printed("shift-bits", source), b"4\n");
}
