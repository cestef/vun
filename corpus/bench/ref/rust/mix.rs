fn main() {
    let mut state: u64 = 88172645463325252;
    let mut hash: u64 = 14695981039346656037;
    for _ in 0..40_000_000u32 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        hash = (hash ^ state).wrapping_mul(1099511628211);
    }
    println!("{hash}");
}
