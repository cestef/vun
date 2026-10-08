const std = @import("std");

extern "c" fn printf(format: [*:0]const u8, ...) c_int;

fn fib(n: u32) u64 {
    return if (n < 2) n else fib(n - 1) +% fib(n - 2);
}

pub fn main(init: std.process.Init.Minimal) void {
    var args = init.args.iterate();
    _ = args.skip();
    const n: u32 = if (args.next()) |text| std.fmt.parseInt(u32, text, 10) catch 34 else 34;
    _ = printf("%llu\n", @as(u64, fib(n)));
}
