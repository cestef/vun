//! RFC 4648 base32 in lowercase without padding: short, and safe in file names on file systems
//! that ignore case.

const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
const BITS: u32 = 5;

#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(BITS as usize));
    let (mut buffer, mut held) = (0u32, 0u32);
    for &b in bytes {
        buffer = (buffer << 8) | u32::from(b);
        held += 8;
        while held >= BITS {
            held -= BITS;
            out.push(char::from(ALPHABET[((buffer >> held) & 31) as usize]));
        }
    }
    if held > 0 {
        out.push(char::from(ALPHABET[((buffer << (BITS - held)) & 31) as usize]));
    }
    out
}

/// Decodes exactly `N` bytes; the unused low bits of the last character must be zero, so that
/// every digest has one spelling.
#[must_use]
pub fn decode<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != (N * 8).div_ceil(BITS as usize) {
        return None;
    }
    let mut out = [0u8; N];
    let (mut buffer, mut held, mut i) = (0u32, 0u32, 0);
    for c in text.bytes() {
        let value = ALPHABET.iter().position(|&a| a == c)?;
        buffer = (buffer << BITS) | u32::try_from(value).ok()?;
        held += BITS;
        if held >= 8 {
            held -= 8;
            *out.get_mut(i)? = u8::try_from((buffer >> held) & 0xff).ok()?;
            i += 1;
        }
    }
    (buffer & ((1 << held) - 1) == 0).then_some(out)
}
