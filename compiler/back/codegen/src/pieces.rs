//! Byte counts as the accesses covering them.

/// The accesses covering `bytes` bytes, widest first, each of one of `sizes` (widest first,
/// `bytes_of` giving each one's bytes, powers of two): each access's size and offset.
///
/// # Panics
/// Never for accesses within memory, whose offsets fit an `i64`.
pub fn pieces<S: Copy>(
    bytes: u64,
    sizes: &[S],
    bytes_of: impl Fn(S) -> u64,
) -> impl Iterator<Item = (S, i64)> {
    let mut done = 0;
    sizes.iter().flat_map(move |&size| {
        let width = bytes_of(size);
        let (count, start) = ((bytes - done) / width, done);
        done += count * width;
        (0..count).map(move |k| (size, i64::try_from(start + k * width).expect("accesses fit memory")))
    })
}
