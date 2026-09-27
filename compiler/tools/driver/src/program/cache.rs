//! What builds keep by their content: the artifacts and objects of a program under a directory
//! named by the artifacts, the C compiler and the profile making the objects; and a stamp per
//! link, named by what it links and how, holding the content of the output it wrote. A build
//! finding them skips compiling and linking again.

use std::path::{Path, PathBuf};

use vun_backend::Artifact;
use vun_hash::Hash;
use vun_link::LinkPlan;
use vun_target::{CCompiler, Name, Profile, TargetSpec};
use vun_trace::tracing;

/// Where link stamps are kept, among the builds.
const LINKED: &str = "linked";

/// The directory holding a program's artifacts and objects.
pub(super) struct Objects;

impl Objects {
    /// The key of the directory holding `artifacts` and the objects `compiler` makes of them
    /// under `profile`.
    pub(super) fn key(artifacts: &[Artifact], compiler: &CCompiler, profile: Profile) -> Hash {
        let contents: Vec<(&Path, &[u8])> =
            artifacts.iter().map(|a| (a.name.as_path(), a.bytes.as_slice())).collect();
        Hash::of_value(&(contents, compiler, profile))
    }
}

/// A link's stamp: where it is kept, named by what the link reads.
pub(super) struct Stamp {
    at: PathBuf,
    output: PathBuf,
}

impl Stamp {
    /// The stamp of linking `plan` for `target` with `linker`, among `builds`. The plan names its
    /// objects by the directories their content keys.
    pub(super) fn of(builds: &Path, linker: Name, target: &TargetSpec, plan: &LinkPlan) -> Self {
        let key = Hash::of_value(&(linker.0, target, plan));
        Self { at: builds.join(LINKED).join(key.digest()), output: plan.output.clone() }
    }

    /// Whether the output is what this link wrote last, so linking again would write it again.
    pub(super) fn holds(&self) -> bool {
        let recorded = std::fs::read_to_string(&self.at).ok().and_then(|text| Hash::parse(&text));
        let written = std::fs::read(&self.output).ok().map(|bytes| Hash::of(&bytes));
        recorded.is_some() && recorded == written
    }

    /// Records the output just written; a stamp not written only costs the next build a link.
    pub(super) fn record(&self) {
        let Ok(bytes) = std::fs::read(&self.output) else { return };
        if let Err(error) = vun_fs::Edit::put(self.at.as_path(), Hash::of(&bytes).to_string()) {
            tracing::debug!(%error, stamp = %self.at.display(), "link stamp not written");
        }
    }
}
