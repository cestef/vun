//! The machine code of a program's functions kept between builds (decision 669): one file per
//! program, toolchain, target and profile under `.vun/code/`, read when code is generated and
//! written again, whole, when a build compiled a function it did not hold. A file holds the code
//! the last build used, so it does not grow with edits; the entries share one stream, so a source
//! file the origins of many functions name is written once.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use vun_backend::Compiled;
use vun_codec::{Context, Decode, Encode, Pools, Writer};
use vun_fs::Edit;
use vun_hash::{Hash, Map};
use vun_query::{Build, Fingerprint};
use vun_target::{Profile, TargetSpec};
use vun_trace::tracing;

/// Where builds keep machine code, among what they keep.
const CODE: &str = "code";

/// A file of kept code, as written.
#[derive(Encode, Decode)]
struct Held {
    entries: Vec<(Fingerprint, Vec<u8>)>,
    pools: Pools,
}

/// The code a build reads and keeps.
pub(super) struct Kept {
    at: PathBuf,
    read: Option<(Context, Map<Fingerprint, Vec<u8>>)>,
    /// The code this build used: read, or compiled anew.
    used: Mutex<Used>,
}

#[derive(Default)]
struct Used {
    /// The keys of the code read.
    read: Vec<Fingerprint>,
    compiled: Vec<(Fingerprint, Compiled<()>)>,
}

impl Kept {
    /// The code kept in `work` for programs of `target` built under `profile` by this toolchain;
    /// none when the toolchain cannot be named.
    pub(super) fn open(work: &Path, root: &Path, target: &TargetSpec, profile: Profile) -> Option<Self> {
        let key = Hash::of_value(&(Build::identity()?, root, target, profile));
        let at = work.join(CODE).join(key.digest());
        let read = std::fs::read(&at).ok().and_then(|bytes| match Held::decoded(&bytes) {
            Ok(held) => Some((Context::new(held.pools), held.entries.into_iter().collect())),
            Err(error) => {
                tracing::debug!(%error, path = %at.display(), "kept code does not read");
                None
            }
        });
        Some(Self { at, read, used: Mutex::default() })
    }

    /// Writes the code this build used, when it compiled code the file did not hold.
    pub(super) fn close(self) {
        let used = self.used.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
        tracing::debug!(read = used.read.len(), compiled = used.compiled.len(), "kept code");
        if used.compiled.is_empty() {
            return;
        }
        let mut writer = Writer::new();
        let mut entries: Vec<(Fingerprint, Vec<u8>)> =
            used.compiled.iter().map(|(key, code)| (*key, writer.encode(code))).collect();
        if let Some((context, held)) = &self.read {
            for key in used.read {
                if let Ok(code) = context.read::<Compiled<()>>(&held[&key]) {
                    entries.push((key, writer.encode(&code)));
                }
            }
        }
        let held = Held { entries, pools: writer.into_pools() };
        if let Err(error) = self.at.put(held.encoded()) {
            tracing::debug!(%error, path = %self.at.display(), "kept code not written");
        }
    }

    fn used(&self) -> std::sync::MutexGuard<'_, Used> {
        self.used.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl vun_backend::Kept for Kept {
    fn get(&self, key: Fingerprint) -> Option<Compiled<()>> {
        let (context, entries) = self.read.as_ref()?;
        let code = context.read::<Compiled<()>>(entries.get(&key)?).ok()?;
        self.used().read.push(key);
        Some(code)
    }

    fn put(&self, key: Fingerprint, code: Compiled<()>) {
        self.used().compiled.push((key, code));
    }
}
