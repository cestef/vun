//! The key releases are signed with: an Ed25519 public key, written `ed25519:<base32>`. A
//! release's manifest is signed over its bytes, the signature published beside it, so that a
//! manifest, and through its hashes every archive, is the one the project released.

use std::error::Error;
use std::fmt;

use ring::signature::{ED25519, UnparsedPublicKey};

/// The public key of the project's releases, as `ed25519:<base32>`.
const RELEASES: &str = include_str!("key.pub");

/// An Ed25519 public key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key([u8; Key::LEN]);

/// A signature that does not verify under the key.
#[derive(Debug)]
pub struct Forged;

impl Key {
    /// Names the key releases are checked against instead of the project's: a mirror signing its
    /// own, or tests.
    pub const ENV: &str = "VUN_RELEASES_KEY";
    /// The algorithm's name in the text form.
    const ALGORITHM: &str = "ed25519";
    const LEN: usize = 32;

    /// The key of the project's releases.
    ///
    /// # Panics
    /// Panics if the key the toolchain was built with does not read, which its tests rule out.
    #[must_use]
    pub fn releases() -> Self {
        Self::parse(RELEASES.trim()).expect("the releases' key reads")
    }

    /// The raw key of `bytes`.
    #[must_use]
    pub fn new(bytes: [u8; Self::LEN]) -> Self {
        Self(bytes)
    }

    /// Reads the text form; `None` for another algorithm or a malformed key.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (algorithm, key) = text.split_once(vun_hash::Hash::SEPARATOR)?;
        (algorithm == Self::ALGORITHM).then_some(())?;
        vun_hash::base32::decode(key).map(Self)
    }

    /// Checks that `signature` signs `message` under this key.
    ///
    /// # Errors
    /// Returns [`Forged`] when it does not.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), Forged> {
        UnparsedPublicKey::new(&ED25519, self.0).verify(message, signature).map_err(|_| Forged)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", Self::ALGORITHM, vun_hash::Hash::SEPARATOR, vun_hash::base32::encode(&self.0))
    }
}

impl fmt::Display for Forged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the signature does not verify under the releases' key")
    }
}

impl Error for Forged {}
