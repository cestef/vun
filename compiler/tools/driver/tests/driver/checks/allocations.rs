//! The checks of allocations: what allocators are given back, and the quarantine holding it.

use vun_target::Check;

use super::{TRAPPED, checked_statuses, each_ends};

/// Memory of a fixed allocator handed out and written, then `then`.
fn handed(then: &str) -> String {
    format!(
        "\
start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    mut buffer: [64]u8 = [0; 64];
    mut fixed := std.mem.alloc.Fixed.init(&mut buffer);
    p := fixed.alloc(8, 8).expect(\"fits\");
    unsafe p.write(5);
    {then}
}};
"
    )
}

#[test]
fn memory_given_back_twice_or_read_once_given_back_traps_through_each_backend() {
    let twice = handed("fixed.free(p, 8, 8);\n    fixed.free(p, 8, 8);\n    std.os.Exit.of(3)");
    let after = handed("fixed.free(p, 8, 8);\n    std.os.Exit.of(unsafe p.read().to(Int(0..=125)))");
    let runs = [
        checked_statuses("checks-twice", &twice, &[Check::Allocations]),
        checked_statuses("checks-after", &after, &[Check::Pointers, Check::Allocations]),
    ];
    for statuses in runs {
        assert!(each_ends(&statuses, TRAPPED), "{statuses:?}");
    }
    // Without the pointer checks, the read goes through.
    assert!(each_ends(&checked_statuses("checks-unread", &after, &[Check::Allocations]), Some(5)));
}

#[test]
fn memory_given_back_that_was_never_handed_out_or_with_another_size_traps_through_each_backend() {
    let cases = [
        ("checks-foreign", "fixed.free(unsafe p.offset(16), 8, 8);", TRAPPED),
        ("checks-size", "fixed.free(p, 16, 8);", TRAPPED),
        ("checks-given", "fixed.free(p, 8, 8);", Some(3)),
    ];
    for (name, free, status) in cases {
        let statuses =
            checked_statuses(name, &handed(&format!("{free}\n    std.os.Exit.of(3)")), &[Check::Allocations]);
        assert!(each_ends(&statuses, status), "{name}: {statuses:?}");
    }
}

/// A program whose allocator hands out a block given back again first, else the next 8 bytes
/// of a buffer, or for more, an address far from any memory that nothing touches. It gives back
/// a block written 5, runs `then`, writes 9 to the block it gets next and exits with the first.
fn reused(then: &str) -> String {
    format!(
        "\
Reuse := struct {{
    next: usize,
    freed: usize,
}};

far: usize = 1 << 40;

#[impl(std.mem.alloc.Allocator(Reuse))]
ReuseAllocator := struct {{
    Error := std.mem.alloc.Error;
    alloc := (self: &mut Reuse, size: usize, align: usize) -> Result(*u8, std.mem.alloc.Error) {{
        mut at := self.freed;
        if at != 0 {{
            self.freed = 0;
        }} else if size > 8 {{
            at = far;
        }} else {{
            at = self.next;
            self.next = (self.next + 8).sat(usize);
        }}
        .ok(unsafe lang.mem.pointer_at(u8, at))
    }};
    free := (self: &mut Reuse, p: *u8, size: usize, align: usize) {{ self.freed = p.address(); }};
}};

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {{
    mut buffer: [64]u8 = [0; 64];
    at := unsafe lang.mem.pointer_to([64]u8, &buffer).address();
    mut reuse: Reuse = .{{ .next = at, .freed = 0 }};
    p := reuse.alloc(8, 8).expect(\"fits\");
    unsafe p.write(5);
    reuse.free(p, 8, 8);
    {then}
    q := reuse.alloc(8, 8).expect(\"fits\");
    unsafe q.write(9);
    std.os.Exit.of(unsafe p.read().to(Int(0..=125)))
}};
"
    )
}

#[test]
fn memory_given_back_stays_poisoned_in_the_quarantine_until_it_is_evicted() {
    let checks = [Check::Pointers, Check::Allocations];
    // Held: the allocator hands out another block, and the read of the first traps.
    assert!(each_ends(&checked_statuses("checks-held", &reused(""), &checks), TRAPPED));
    assert!(each_ends(&checked_statuses("checks-held-unread", &reused(""), &[Check::Allocations]), Some(5)));
    // A block past the quarantine's bound given back evicts the first, which is handed out again.
    let evicted = reused(
        "big := reuse.alloc(lang.check.quarantine, 8).expect(\"fits\");\n    reuse.free(big, lang.check.quarantine, 8);",
    );
    assert!(each_ends(&checked_statuses("checks-evicted", &evicted, &checks), Some(9)));
}
