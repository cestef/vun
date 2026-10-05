//! What the package commands and fetching report.

use vun_build::manifest::Version;
use vun_data::Object;
use vun_hash::Hash;
use vun_intern::Symbol;
use vun_term::{Bytes, Noun, Record, Shell, Status, Style, Text, Verb};

use super::{HashText, Shown};
use vun_driver::packages::Fetched;

const PACKAGE: Noun = Noun { one: "package", many: "packages" };

/// A dependency downloaded into the store.
pub(in crate::commands) struct Downloaded {
    pub(in crate::commands) name: Symbol,
    pub(in crate::commands) hash: Hash,
    pub(in crate::commands) url: String,
}

impl Record for Downloaded {
    fn kind(&self) -> &'static str {
        "downloaded"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, self.name.as_str().to_owned())
            .plain(" ")
            .append(self.hash.text(Shown::Short))
            .plain(" from ")
            .plain(self.url.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.name.as_str())
            .with("hash", self.hash.to_string())
            .with("url", self.url.clone())
    }
}

impl Status for Downloaded {
    fn verb(&self) -> Verb {
        Verb::good("Downloaded")
    }
}

/// An outcome of fetching, as a command line reports it.
pub(crate) trait Report {
    /// Reports this outcome of fetching `name` at `hash`, when a download happened.
    fn report(&self, name: Symbol, hash: Hash, shell: &mut Shell<'_>);
}

impl Report for Fetched {
    fn report(&self, name: Symbol, hash: Hash, shell: &mut Shell<'_>) {
        if let Self::Stored { url } = self {
            shell.status(&Downloaded { name, hash, url: url.to_string() });
        }
    }
}

/// Where an added dependency comes from.
pub(in crate::commands) enum Source {
    Path(String),
    Library(String),
    Archive { version: Version, hash: Hash },
}

/// A dependency declared in the manifest.
pub(in crate::commands) struct Added {
    pub(in crate::commands) name: String,
    pub(in crate::commands) source: Source,
}

impl Record for Added {
    fn kind(&self) -> &'static str {
        "added"
    }

    fn text(&self) -> Text {
        match &self.source {
            Source::Path(path) => {
                Text::of(Style::Strong, self.name.clone()).plain(" at ").plain(path.clone())
            }
            Source::Library(path) => {
                Text::of(Style::Strong, self.name.clone()).plain(" from the library at ").plain(path.clone())
            }
            Source::Archive { version, hash } => Text::of(Style::Strong, format!("{} {version}", self.name))
                .plain(" ")
                .append(hash.text(Shown::Short)),
        }
    }

    fn fields(&self) -> Object<'static> {
        let object = Object::new().with("name", self.name.clone());
        match &self.source {
            Source::Path(path) => object.with("path", path.clone()),
            Source::Library(path) => object.with("library", path.clone()),
            Source::Archive { version, hash } => {
                object.with("version", version.to_string()).with("hash", hash.to_string())
            }
        }
    }
}

impl Status for Added {
    fn verb(&self) -> Verb {
        Verb::good("Added")
    }
}

/// A dependency raised to a newer release.
pub(in crate::commands) struct Updated {
    pub(in crate::commands) name: String,
    pub(in crate::commands) from: Version,
    pub(in crate::commands) to: Version,
    pub(in crate::commands) hash: Hash,
}

impl Record for Updated {
    fn kind(&self) -> &'static str {
        "updated"
    }

    fn text(&self) -> Text {
        Text::of(Style::Strong, format!("{} {}", self.name, self.from))
            .plain(format!(" to {} ", self.to))
            .append(self.hash.text(Shown::Short))
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.name.clone())
            .with("from", self.from.to_string())
            .with("to", self.to.to_string())
            .with("hash", self.hash.to_string())
    }
}

impl Status for Updated {
    fn verb(&self) -> Verb {
        Verb::good("Updated")
    }
}

/// Every dependency at the newest release its indexes list.
pub(in crate::commands) struct UpToDate;

impl Record for UpToDate {
    fn kind(&self) -> &'static str {
        "up_to_date"
    }

    fn text(&self) -> Text {
        Text::from("every dependency is at the newest release its indexes list")
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
    }
}

impl Status for UpToDate {
    fn verb(&self) -> Verb {
        Verb::good("Fresh")
    }
}

/// The package's archive, written.
pub(in crate::commands) struct Packed {
    pub(in crate::commands) name: String,
    pub(in crate::commands) version: Version,
    pub(in crate::commands) hash: Hash,
    pub(in crate::commands) path: String,
}

impl Record for Packed {
    fn kind(&self) -> &'static str {
        "packed"
    }

    fn text(&self) -> Text {
        let text = Text::of(Style::Strong, format!("{} {}", self.name, self.version))
            .plain(" ")
            .append(self.hash.text(Shown::Short));
        text.plain(" to ").plain(self.path.clone())
    }

    fn fields(&self) -> Object<'static> {
        Object::new()
            .with("name", self.name.clone())
            .with("version", self.version.to_string())
            .with("hash", self.hash.to_string())
            .with("path", self.path.clone())
    }
}

impl Status for Packed {
    fn verb(&self) -> Verb {
        Verb::good("Packed")
    }
}

/// What collecting the store removed and kept.
pub(in crate::commands) struct Collected {
    pub(in crate::commands) removed: usize,
    pub(in crate::commands) bytes: u64,
    pub(in crate::commands) kept: usize,
}

impl Record for Collected {
    fn kind(&self) -> &'static str {
        "collected"
    }

    fn text(&self) -> Text {
        Text::from(format!("{}, {}; kept {}", PACKAGE.count(self.removed), Bytes(self.bytes), self.kept))
    }

    fn fields(&self) -> Object<'static> {
        Object::new().with("removed", self.removed).with("bytes", self.bytes).with("kept", self.kept)
    }
}

impl Status for Collected {
    fn verb(&self) -> Verb {
        Verb::good("Removed")
    }
}
