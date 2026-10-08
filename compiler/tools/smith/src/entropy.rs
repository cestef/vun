//! Where choices come from: a seed, or the bytes a fuzzer mutates. Both give the same stream of
//! draws to the generator, so a program is reproduced from either and a mutation of the bytes is
//! a small change of the program.

/// A source of choices. Bytes that run out give the first choice of everything, which ends
/// whatever the generator was still building.
pub struct Entropy {
    source: Source,
}

enum Source {
    Seeded(u64),
    Bytes { bytes: Vec<u8>, at: usize },
}

impl Entropy {
    /// The stream a seed makes: splitmix64.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self { source: Source::Seeded(seed) }
    }

    /// The stream `bytes` make, read in order.
    #[must_use]
    pub fn bytes(bytes: &[u8]) -> Self {
        Self { source: Source::Bytes { bytes: bytes.to_vec(), at: 0 } }
    }

    /// Whether a stream of bytes has no more to give.
    #[must_use]
    pub fn exhausted(&self) -> bool {
        matches!(&self.source, Source::Bytes { bytes, at } if *at >= bytes.len())
    }

    fn word(&mut self) -> u64 {
        match &mut self.source {
            Source::Seeded(state) => {
                *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
                let mut z = *state;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                z ^ (z >> 31)
            }
            Source::Bytes { bytes, at } => Self::read(bytes, at, size_of::<u64>()),
        }
    }

    /// The next `width` bytes, fewer at the end, as a number.
    fn read(bytes: &[u8], at: &mut usize, width: usize) -> u64 {
        let taken = bytes.get(*at..).unwrap_or_default();
        let taken = &taken[..taken.len().min(width)];
        *at += taken.len();
        taken.iter().rev().fold(0, |word, &b| (word << 8) | u64::from(b))
    }

    /// A choice among `n`: below `n`, or 0 when `n` is 0 or 1.
    ///
    /// # Panics
    /// Never: the remainder is below `n`, a `usize`.
    pub fn below(&mut self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        // A byte stream spends as many bytes as the choice needs, so that a mutation of one
        // byte changes one choice.
        let word = match &mut self.source {
            Source::Bytes { bytes, at } => {
                let width = (usize::BITS - (n - 1).leading_zeros()).div_ceil(u8::BITS) as usize;
                Self::read(bytes, at, width)
            }
            Source::Seeded(_) => self.word(),
        };
        usize::try_from(word % n as u64).expect("the remainder is below `n`, a `usize`")
    }

    /// Whether a chance of `numerator` in `denominator` came up.
    pub fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
        self.below(denominator) < numerator
    }

    /// One of `items`.
    ///
    /// # Panics
    /// Panics when there are none.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// The index of one of `weights`, by weight; none of weight 0.
    ///
    /// # Panics
    /// Panics when every weight is 0.
    pub fn weighted(&mut self, weights: &[usize]) -> usize {
        let total: usize = weights.iter().sum();
        let mut at = self.below(total);
        weights
            .iter()
            .position(|&w| {
                let hit = at < w;
                at = at.saturating_sub(w);
                hit
            })
            .expect("a weight is not 0")
    }

    /// A number from `lo` to `hi`, both included.
    pub fn between(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }

    /// A whole word: where a literal's bits come from.
    pub fn bits(&mut self) -> u64 {
        self.word()
    }
}
