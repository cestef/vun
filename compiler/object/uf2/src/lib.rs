//! UF2 files (Microsoft's USB Flashing Format): a program as 512-byte blocks, each carrying the
//! bytes for one address, which a boot ROM that shows itself as a mass storage volume writes
//! where they say as it receives them, whatever order the host's file system copies them in.
//! Each block holds its own number and the count, and the family of chip it is for.

mod block;
#[cfg(test)]
mod tests;

pub use block::{Block, Malformed};

use vun_bytes::Out;

/// The bytes of a program at an address of the chip's memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece<'a> {
    pub address: u64,
    pub bytes: &'a [u8],
}

/// The chip a file is for, by the number the format's registry gives its family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uf2 {
    pub family: u32,
}

/// A piece starts or ends past the 32 bits of address a block holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unaddressable {
    pub address: u64,
}

impl std::fmt::Display for Unaddressable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "address {:#x} does not fit the 32 bits a block holds", self.address)
    }
}

impl std::error::Error for Unaddressable {}

impl Uf2 {
    /// The file holding `pieces`, each cut into blocks of [`Block::PAYLOAD`] bytes at the
    /// addresses they have, in the order given.
    ///
    /// # Errors
    /// Returns the first address a block cannot hold.
    pub fn write(self, pieces: &[Piece<'_>]) -> Result<Vec<u8>, Unaddressable> {
        let mut blocks = Vec::new();
        for piece in pieces {
            for (k, chunk) in piece.bytes.chunks(Block::PAYLOAD).enumerate() {
                let address = piece.address + (k * Block::PAYLOAD) as u64;
                let address = u32::try_from(address).map_err(|_| Unaddressable { address })?;
                blocks.push((address, chunk));
            }
        }
        let count = u32::try_from(blocks.len()).expect("a program has fewer blocks than 32 bits count");
        let mut out = Out::default();
        for (number, (address, chunk)) in (0..).zip(blocks) {
            Block { address, number, count, family: self.family, payload: chunk.to_vec() }.put(&mut out);
        }
        Ok(out.0)
    }

    /// The blocks of the file `bytes` hold, checked as the format asks: whole, in their magics,
    /// numbered in order, counting the file's blocks, and of one family.
    ///
    /// # Errors
    /// Returns the first way a block breaks the format.
    pub fn read(bytes: &[u8]) -> Result<(Self, Vec<Block>), Malformed> {
        Block::read_all(bytes)
    }
}
