//! Blocks of WebAssembly instructions (spec 07, section 6) through the wasm backend: the program
//! written as instructions runs as the same program written in Vun.

use vun_tool::Executable;

use super::{build_source, run};

/// Each function is a block, and the exit status tells which ones computed what Vun does: 42 from
/// an addition, 2 from `local_tee` and `drop`, 6 from a 64-bit shift, 1 from the size of memory,
/// 1 from an atomic addition, 2 from an atomic compare and exchange, 3 from a loop that adds 4, 3,
/// 2 and 1, 5 and 6 from float constants, 4 from the two arms of an `if`, 7 from a block giving a
/// value, 8 from a branch table, 10 from a call, 11 from the stack pointer, 12 from vectors and 1 from a call through the table and 2 from a vector bound to locals.
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

tri := (n: i32) -> i32 {
    mut left: i32 = n;
    mut total: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        block();
        loop();
        local_get(inout(left));
        i32_eqz();
        br_if(1);
        local_get(inout(total));
        local_get(inout(left));
        i32_add();
        local_set(inout(total));
        local_get(inout(left));
        i32_const(1);
        i32_sub();
        local_set(inout(left));
        br(0);
        end();
        end();
    });
    total
};

pick := (flag: i32, cell: &mut i32) {
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(flag));
        if_();
        local_get(in(cell));
        i32_const(7);
        i32_store(2, 0);
        else_();
        local_get(in(cell));
        i32_const(9);
        i32_store(2, 0);
        end();
    });
};

half := (x: f32) -> f32 {
    mut r: f32 = 0.0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(x));
        f32_const(0x3f000000);
        f32_mul();
        local_set(out(r));
    });
    r
};

more := (x: f64) -> f64 {
    mut r: f64 = 0.0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(x));
        f64_const(1.5);
        f64_add();
        local_set(out(r));
    });
    r
};

sign := (x: i32) -> i32 {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        block(yield_i32);
        i32_const(100);
        local_get(in(x));
        i32_const(0);
        i32_lt_s();
        br_if(0);
        drop();
        i32_const(200);
        end();
        local_set(out(r));
    });
    r
};

dispatch := (n: i32) -> i32 {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        block();
        block();
        block();
        local_get(in(n));
        br_table(0, 1, 2);
        end();
        i32_const(10);
        local_set(out(r));
        br(1);
        end();
        i32_const(20);
        local_set(out(r));
        br(0);
        end();
    });
    r
};

twice := (x: i32) -> i32 {
    mut y: i32 = x;
    y +%= x;
    y
};

call_twice := (x: i32) -> i32 {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(x));
        call(twice);
        local_set(out(r));
    });
    r
};

call_through := (f: (x: i32) -> i32, x: i32) -> i32 {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(x));
        local_get(in(f));
        call_indirect(twice);
        local_set(out(r));
    });
    r
};

stack_top := () -> u32 {
    mut sp: u32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        global_get(stack_pointer);
        global_get(stack_pointer);
        global_set(stack_pointer);
        local_set(out(sp));
    });
    sp
};

lanes := (x: i32) -> i32 {
    mut r: i32 = 0;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(x));
        i32x4_splat();
        v128_const(0x0000000200000001, 0x0000000400000003);
        i32x4_add();
        i32x4_extract_lane(3);
        local_set(out(r));
    });
    r
};

V: type = lang.simd.Vector(4, i32);

doubled := (v: V) -> V {
    mut r: V = v;
    unsafe lang.arch.asm({
        ..lang.arch.wasm32.names;
        local_get(in(v));
        local_get(in(v));
        i32x4_add();
        local_set(out(r));
    });
    r
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
    if tri(4) == 10 { status +%= 3; }
    if half(8.0) == 4.0 { status +%= 5; }
    if more(2.0) == 3.5 { status +%= 6; }
    if sign(-4) == 100 and sign(4) == 200 { status +%= 7; }
    if dispatch(0) == 10 and dispatch(1) == 20 and dispatch(5) == 0 { status +%= 8; }
    if call_twice(21) == 42 { status +%= 10; }
    if call_through(twice, 8) == 16 { status +%= 1; }
    if stack_top() > 0 { status +%= 11; }
    if lanes(10) == 14 { status +%= 12; }
    mut held: [4]i32 = [0; 4];
    doubled(V.splat(5)).store(&mut held);
    if held[3] == 10 { status +%= 2; }
    mut then: i32 = 0;
    mut otherwise: i32 = 0;
    pick(1, &mut then);
    pick(0, &mut otherwise);
    if then == 7 and otherwise == 9 { status +%= 4; }
    std.os.Exit.of(status.to(Int(0..=125)))
};
";

#[test]
fn a_block_of_wasm_instructions_computes_what_the_program_in_vun_does() {
    let Some(wasmtime) = Executable::search("wasmtime") else { return };
    let (_fixture, linked) = build_source("asm", PROGRAM, None);
    assert_eq!(run(&wasmtime, &linked).0, Some(123));
}
