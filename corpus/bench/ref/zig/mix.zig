extern "c" fn printf(format: [*:0]const u8, ...) c_int;

pub fn main() void {
    var state: u64 = 88172645463325252;
    var hash: u64 = 14695981039346656037;
    var i: u32 = 0;
    while (i < 40_000_000) : (i += 1) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        hash = (hash ^ state) *% 1099511628211;
    }
    _ = printf("%llu\n", hash);
}
