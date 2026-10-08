extern "c" fn printf(format: [*:0]const u8, ...) c_int;

fn fib(n: u32) u64 {
    return if (n < 2) n else fib(n - 1) +% fib(n - 2);
}

pub fn main() void {
    _ = printf("%llu\n", @as(u64, fib(34)));
}
