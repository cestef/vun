//! A hash persists as its algorithm's position in [`Algorithm::ALL`], then its digest.

use vun_codec::{Decode, Encode, Malformed, Reader, Writer};

use crate::{Algorithm, Hash};

impl Encode for Hash {
    fn encode(&self, w: &mut Writer<'_>) {
        let position =
            Algorithm::ALL.iter().position(|&a| a == self.algorithm).expect("every algorithm is listed");
        position.encode(w);
        self.digest.encode(w);
    }
}

impl Decode for Hash {
    fn decode(r: &mut Reader<'_>) -> Result<Self, Malformed> {
        let position = usize::decode(r)?;
        let &algorithm = Algorithm::ALL.get(position).ok_or(Malformed::Tag(position as u64))?;
        Ok(Self { algorithm, digest: Decode::decode(r)? })
    }
}
