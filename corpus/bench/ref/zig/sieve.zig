extern "c" fn printf(format: [*:0]const u8, ...) c_int;

const N = 4_000_000;

pub fn main() void {
    var composite: [N]bool = undefined;
    @memset(&composite, false);
    var count: u32 = 0;
    var i: usize = 2;
    while (i < N) : (i += 1) {
        if (!composite[i]) {
            count +%= 1;
            var j: usize = i *% i;
            while (j < N) : (j +%= i) composite[j] = true;
        }
    }
    _ = printf("%u\n", count);
}
