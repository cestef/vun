//! A block: 32 bytes of header, 476 for the payload, 4 of the end's magic.

use std::fmt;

use vun_bytes::{In, Out};

use crate::Uf2;

const FIRST_MAGIC: u32 = 0x0a32_4655;
const SECOND_MAGIC: u32 = 0x9e5d_5157;
const END_MAGIC: u32 = 0x0ab1_6f30;

/// Says the family's number is in the field the size would take in a file without one.
const FAMILY_PRESENT: u32 = 0x2000;

/// The bytes the header takes before the payload's room, and the room.
const HEADER: usize = 32;
const ROOM: usize = 476;

/// A block of a file: where its payload goes, its place among the file's blocks, the family of
/// chip it is for, and the bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub address: u32,
    pub number: u32,
    pub count: u32,
    pub family: u32,
    pub payload: Vec<u8>,
}

impl Block {
    /// The bytes of payload a block carries: what a chip's flash writes at once.
    pub const PAYLOAD: usize = 256;

    /// The size of a block, which a file holds a whole number of.
    pub const SIZE: usize = 512;

    pub(crate) fn put(&self, out: &mut Out) {
        for word in [FIRST_MAGIC, SECOND_MAGIC, FAMILY_PRESENT, self.address] {
            out.u32(word);
        }
        out.u32(u32::try_from(self.payload.len()).expect("a payload fits its room"));
        for word in [self.number, self.count, self.family] {
            out.u32(word);
        }
        out.bytes(&self.payload);
        out.zeros(ROOM - self.payload.len());
        out.u32(END_MAGIC);
    }

    fn take(input: &mut In<'_>, at: usize) -> Result<Self, Malformed> {
        let word = |input: &mut In<'_>| input.u32().map_err(|_| Malformed::Truncated);
        let magics = [word(input)?, word(input)?];
        let flags = word(input)?;
        let (address, size) = (word(input)?, word(input)?);
        let (number, count, family) = (word(input)?, word(input)?, word(input)?);
        let room = input.take(ROOM as u64).map_err(|_| Malformed::Truncated)?;
        let end = word(input)?;
        if magics != [FIRST_MAGIC, SECOND_MAGIC] || end != END_MAGIC {
            return Err(Malformed::Magic { block: at });
        }
        if flags & FAMILY_PRESENT == 0 {
            return Err(Malformed::Family { block: at });
        }
        let payload = room.get(..size as usize).ok_or(Malformed::Size { block: at, size })?.to_vec();
        Ok(Self { address, number, count, family, payload })
    }

    pub(crate) fn read_all(bytes: &[u8]) -> Result<(Uf2, Vec<Self>), Malformed> {
        if bytes.len() % Self::SIZE != 0 {
            return Err(Malformed::Truncated);
        }
        let mut input = In::at(bytes, 0);
        let blocks = (0..bytes.len() / Self::SIZE)
            .map(|at| Self::take(&mut input, at))
            .collect::<Result<Vec<_>, _>>()?;
        let family = blocks.first().map_or(0, |b| b.family);
        for (at, block) in blocks.iter().enumerate() {
            if block.family != family {
                return Err(Malformed::Family { block: at });
            }
            if block.number as usize != at || block.count as usize != blocks.len() {
                return Err(Malformed::Numbering { block: at });
            }
        }
        debug_assert_eq!(HEADER + ROOM + 4, Self::SIZE);
        Ok((Uf2 { family }, blocks))
    }
}

/// Why bytes are not a UF2 file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// They are not a whole number of blocks.
    Truncated,
    /// A block lacks the format's magic numbers.
    Magic { block: usize },
    /// A block does not say which family it is for, or is for another than the first's.
    Family { block: usize },
    /// A block claims more payload than it has room for.
    Size { block: usize, size: u32 },
    /// A block's number or the count it holds is not its place among the file's.
    Numbering { block: usize },
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("not a whole number of blocks"),
            Self::Magic { block } => write!(f, "block {block} lacks the format's magic numbers"),
            Self::Family { block } => write!(f, "block {block} is not for the family of the first"),
            Self::Size { block, size } => write!(f, "block {block} claims {size} bytes of payload"),
            Self::Numbering { block } => write!(f, "block {block} is not numbered by its place"),
        }
    }
}

impl std::error::Error for Malformed {}
