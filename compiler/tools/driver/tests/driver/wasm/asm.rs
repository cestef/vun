//! Blocks of WebAssembly instructions (spec 07, section 6) through the wasm backend: the program
//! written as instructions runs as the same program written in Vun.

use vun_tool::Executable;

use super::{build_source, run};

/// Each function is a block, and the exit status tells which ones computed what Vun does: 42 from
/// an addition, 2 from `local_tee` and `drop`, 6 from a 64-bit shift, 1 from the size of memory,
/// 1 from an atomic addition and 2 from an atomic compare and exchange.
const PROGRAM: &str = r"
add := (a: i32, b: i32) -> i32 {
    mut sum: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(a));
        local_get(in(b));
        i32_add();
        local_set(out(sum));
    });
    sum
};

bump := (n: i32) -> i32 {
    mut m: i32 = n;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(n));
        i32_const(1);
        i32_add();
        local_tee(inout(m));
        drop();
    });
    m
};

double := (a: i32) -> i64 {
    mut wide: i64 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(a));
        i64_extend_i32_u();
        i64_const(1);
        i64_shl();
        local_set(out(wide));
    });
    wide
};

pages := () -> i32 {
    mut count: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        memory_size();
        local_set(out(count));
    });
    count
};

fetch_add := (cell: &mut i32, n: i32) -> i32 {
    mut old: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(cell));
        local_get(in(n));
        i32_atomic_rmw_add(2, 0);
        local_set(out(old));
    });
    old
};

exchange := (cell: &mut i32, expected: i32, new: i32) -> i32 {
    mut old: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(cell));
        local_get(in(expected));
        local_get(in(new));
        i32_atomic_rmw_cmpxchg(2, 0);
        local_set(out(old));
    });
    old
};

start := std.os.start(main);
main := (sys: &mut std.os.System) -> std.os.Exit {
    mut status: u32 = 0;
    if add(30, 12) == 42 { status = 42; }
    if bump(1) == 2 { status +%= 2; }
    if double(3) == 6 { status +%= 6; }
    if pages() > 0 { status +%= 1; }
    mut cell: i32 = 40;
    if fetch_add(&mut cell, 2) == 40 and cell == 42 { status +%= 1; }
    if exchange(&mut cell, 42, 7) == 42 and cell == 7 { status +%= 2; }
    std.os.Exit.of(status.to(Int(0..=125)))
};
";

#[test]
fn a_block_of_wasm_instructions_computes_what_the_program_in_vun_does() {
    let Some(wasmtime) = Executable::search("wasmtime") else { return };
    let (_fixture, linked) = build_source("asm", PROGRAM, None);
    assert_eq!(run(&wasmtime, &linked).0, Some(54));
}
