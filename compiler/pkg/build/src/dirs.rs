//! The user's directories, where the platform has them (XDG on Unix, including macOS, as
//! command-line tools expect; the known folders on Windows). WASI has no home: a compiler running
//! there has none, and keeps what it would keep there beside the files it is given.

use std::path::PathBuf;

pub(crate) struct Dirs {
    pub(crate) home: PathBuf,
    pub(crate) data: PathBuf,
    pub(crate) config: PathBuf,
    pub(crate) cache: PathBuf,
}

impl Dirs {
    /// The running user's; `None` for a user without a home.
    #[cfg(not(target_os = "wasi"))]
    pub(crate) fn user() -> Option<Self> {
        use etcetera::BaseStrategy;
        let base = etcetera::choose_base_strategy().ok()?;
        Some(Self {
            home: base.home_dir().to_path_buf(),
            data: base.data_dir(),
            config: base.config_dir(),
            cache: base.cache_dir(),
        })
    }

    #[cfg(target_os = "wasi")]
    pub(crate) fn user() -> Option<Self> {
        None
    }
}
