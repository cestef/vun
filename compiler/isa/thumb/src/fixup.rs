//! References to labels: a branch's distance, counted in halfwords from the instruction's address
//! plus 4 (the manual's `PC`), written into the branch's fields, which the 32-bit forms scatter;
//! and a literal's, counted in bytes from that address rounded down to a word.

use vun_isa::Unpatched;
use vun_isa::imm::{field, fits_signed};

/// How far a branch reads the program counter ahead of itself: a relocation of a branch's field
/// adds it back.
pub const PC_AHEAD: i64 = 4;
/// The bytes a branch's distance counts in.
const HALFWORD: i64 = 2;
/// The bytes a literal's distance is counted from a multiple of.
const WORD: i64 = 4;

/// A reference to a label, by the field its branch has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fixup {
    /// Eight bits at 0: a 16-bit `b<c>`.
    NarrowCond,
    /// Eleven bits at 0: a 16-bit `b`.
    Narrow,
    /// Six bits, forward only, `i` at 9 and `imm5` at 3: `cbz`, `cbnz`.
    Compare,
    /// Twenty bits, `S:J2:J1:imm6:imm11`: a 32-bit `b<c>`.
    Cond,
    /// Twenty-four bits, `S:J1:J2:imm10:imm11` with the `J`s inverted by `S`: `b.w`, `bl`.
    Jump,
    /// Twelve bits of bytes and their sign in `U` at 23: `ldr` of a literal.
    Literal,
}

impl Fixup {
    /// The bits of the distance, in halfwords, it holds, and whether its branch is 32 bits.
    const fn span(self) -> (u32, bool) {
        match self {
            Self::NarrowCond => (8, false),
            Self::Narrow => (11, false),
            Self::Compare => (6, false),
            Self::Cond => (20, true),
            Self::Jump => (24, true),
            Self::Literal => (12, true),
        }
    }

    /// Whether its branch is 32 bits.
    #[must_use]
    pub const fn wide(self) -> bool {
        self.span().1
    }

    /// The fields holding a distance of `v` halfwords.
    #[must_use]
    pub fn fields(self, v: i64) -> u32 {
        let bits = |at: u32, n: u32| field(v, at, n);
        match self {
            Self::NarrowCond => bits(0, 8),
            Self::Narrow => bits(0, 11),
            Self::Compare => bits(5, 1) << 9 | bits(0, 5) << 3,
            Self::Cond => {
                bits(19, 1) << 26 | bits(11, 6) << 16 | bits(17, 1) << 13 | bits(18, 1) << 11 | bits(0, 11)
            }
            Self::Jump => {
                let s = bits(23, 1);
                let j1 = !(bits(22, 1) ^ s) & 1;
                let j2 = !(bits(21, 1) ^ s) & 1;
                s << 26 | bits(11, 10) << 16 | j1 << 13 | j2 << 11 | bits(0, 11)
            }
            Self::Literal => u32::from(v >= 0) << 23 | field(v.abs(), 0, 12),
        }
    }

    /// The distance in its units from a reference at `at` to `target`, if its field holds it.
    #[must_use]
    pub fn reach(self, at: u64, target: u64) -> Option<i64> {
        let pc = i128::from(at) + i128::from(PC_AHEAD);
        let (from, unit) = match self {
            Self::Literal => (pc - pc % i128::from(WORD), 1),
            _ => (pc, HALFWORD),
        };
        let distance = i64::try_from(i128::from(target) - from).ok()?;
        let v = distance / unit;
        let (bits, _) = self.span();
        let fits = match self {
            Self::Compare => (0..1 << bits).contains(&v),
            Self::Literal => v.unsigned_abs() < 1 << bits,
            _ => fits_signed(v, bits, 64),
        };
        (distance % unit == 0 && fits).then_some(v)
    }

    /// Writes into `code` the distance from the branch at `at` to `target`.
    ///
    /// # Errors
    /// Returns when the distance does not fit the field.
    ///
    /// # Panics
    /// When `at` is past the address space.
    pub fn patch(self, code: &mut [u8], at: u64, target: u64) -> Result<(), Unpatched> {
        let distance = i64::try_from(i128::from(target) - i128::from(at)).unwrap_or(i64::MAX);
        let v = self.reach(at, target).ok_or(Unpatched::Range { at, distance })?;
        let at = usize::try_from(at).expect("code fits memory");
        let fields = self.fields(v);
        let halves = if self.wide() { 2 } else { 1 };
        let word = read(code, at, halves) | fields;
        write(code, at, halves, word);
        Ok(())
    }
}

/// The `halves` halfwords at `at`, the first in the high bits.
fn read(code: &[u8], at: usize, halves: usize) -> u32 {
    (0..halves).fold(0, |word, k| {
        let half = u16::from_le_bytes([code[at + 2 * k], code[at + 2 * k + 1]]);
        word << 16 | u32::from(half)
    })
}

fn write(code: &mut [u8], at: usize, halves: usize, word: u32) {
    for k in 0..halves {
        let shift = 16 * (halves - 1 - k);
        let half = u16::try_from(word >> shift & 0xffff).expect("a halfword");
        code[at + 2 * k..at + 2 * k + 2].copy_from_slice(&half.to_le_bytes());
    }
}

/// The bytes of an encoding, a halfword or two, the first halfword first.
#[must_use]
pub fn bytes(word: u32, wide: bool) -> Vec<u8> {
    let halves = if wide { 2 } else { 1 };
    let mut code = vec![0; 2 * halves];
    write(&mut code, 0, halves, word);
    code
}
