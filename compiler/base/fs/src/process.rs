//! The running process, as the files it writes are told from those of another by.

use std::hash::{BuildHasher, RandomState};
use std::sync::LazyLock;

/// A number no other process running beside this one has: its id where the platform gives one,
/// and a random one under WASI, which gives none (`std::process::id` panics there).
pub static PROCESS: LazyLock<u32> = LazyLock::new(|| {
    if cfg!(target_os = "wasi") {
        // Only the low half of the hash is kept.
        #[allow(clippy::cast_possible_truncation)]
        let random = RandomState::new().hash_one(()) as u32;
        random
    } else {
        std::process::id()
    }
});
