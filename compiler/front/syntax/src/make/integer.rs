//! Integer literals.

/// An integer literal by its sign and magnitude, so that every integer of 128 bits is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Integer {
    pub negative: bool,
    pub magnitude: u128,
}

impl From<i128> for Integer {
    fn from(n: i128) -> Self {
        Self { negative: n < 0, magnitude: n.unsigned_abs() }
    }
}

impl From<u128> for Integer {
    fn from(magnitude: u128) -> Self {
        Self { negative: false, magnitude }
    }
}

impl From<u64> for Integer {
    fn from(n: u64) -> Self {
        u128::from(n).into()
    }
}

impl From<i64> for Integer {
    fn from(n: i64) -> Self {
        i128::from(n).into()
    }
}

impl From<u32> for Integer {
    fn from(n: u32) -> Self {
        u128::from(n).into()
    }
}

impl From<i32> for Integer {
    fn from(n: i32) -> Self {
        i128::from(n).into()
    }
}

impl From<usize> for Integer {
    fn from(n: usize) -> Self {
        u128::try_from(n).expect("a size is below 2^128").into()
    }
}

impl std::fmt::Display for Integer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        write!(f, "{sign}{}", self.magnitude)
    }
}
