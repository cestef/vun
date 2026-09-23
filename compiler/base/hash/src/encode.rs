//! A hash persists as its algorithm's position in [`Algorithm::ALL`], then its digest.

use overt_codec::{Decode, Encode, Malformed};

use crate::{Algorithm, Hash};

impl Encode for Hash {
    fn encode(&self, out: &mut Vec<u8>) {
        let position =
            Algorithm::ALL.iter().position(|&a| a == self.algorithm).expect("every algorithm is listed");
        position.encode(out);
        self.digest.encode(out);
    }
}

impl Decode for Hash {
    fn decode(input: &mut &[u8]) -> Result<Self, Malformed> {
        let position = usize::decode(input)?;
        let &algorithm = Algorithm::ALL.get(position).ok_or(Malformed::Tag(position as u64))?;
        Ok(Self { algorithm, digest: Decode::decode(input)? })
    }
}
