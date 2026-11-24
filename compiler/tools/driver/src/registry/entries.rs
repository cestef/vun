//! The components a registry holds, listed across its kinds: what each is, where it comes from
//! and what it handles.

use std::fmt;
use std::str::FromStr;

use vun_target::{Names, Readiness};
use vun_variants::Variants;

use super::{Provider, Registry};

/// What a component is for: the kinds of components a toolchain has (spec 12, section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Variants)]
pub enum Kind {
    Backend,
    Linker,
    Scheme,
    Server,
    Binder,
    Formatter,
    Documenter,
    Renderer,
}

impl Kind {
    /// How the kind begins the id of a component, as its crate's name does.
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Linker => "link",
            other => other.name(),
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::Linker => "linker",
            Self::Scheme => "scheme",
            Self::Server => "server",
            Self::Binder => "binder",
            Self::Formatter => "formatter",
            Self::Documenter => "documenter",
            Self::Renderer => "renderer",
        }
    }
}

/// A component's kind and the name its interface gives it; a release names its pieces by the
/// kind's prefix and the name, `backend-cranelift` and `link-lld`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Id {
    pub kind: Kind,
    pub name: String,
}

impl Id {
    const SEPARATOR: char = '-';
}

/// A name that is no kind's prefix and a name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unkinded;

impl FromStr for Id {
    type Err = Unkinded;

    fn from_str(text: &str) -> Result<Self, Unkinded> {
        Kind::ALL
            .iter()
            .find_map(|&kind| {
                let name = text.strip_prefix(kind.prefix())?.strip_prefix(Self::SEPARATOR)?;
                Some(Self { kind, name: name.to_owned() })
            })
            .ok_or(Unkinded)
    }
}

impl fmt::Display for Unkinded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the name is not the prefix of a component kind and a name")
    }
}

impl std::error::Error for Unkinded {}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.kind.prefix(), Self::SEPARATOR, self.name)
    }
}

/// A component of a registry.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: Id,
    pub from: Provider,
    pub readiness: Readiness,
    /// The architectures a backend generates code for, or the object formats a linker links.
    pub handles: Names,
    /// What a backend emits, or what a linker builds programs as.
    pub outputs: Vec<&'static str>,
}

impl Entry {
    /// A component of a kind that has no facts but its name.
    fn plain(kind: Kind, name: &str, from: &Provider) -> Self {
        Self {
            id: Id { kind, name: name.to_owned() },
            from: from.clone(),
            readiness: Readiness::Ready,
            handles: Names::Any,
            outputs: Vec::new(),
        }
    }
}

impl Registry {
    /// Every component, backends and linkers first.
    #[must_use]
    pub fn entries(&self) -> Vec<Entry> {
        let backends = self.backends().map(vun_backend::Backend::info).map(|i| Entry {
            id: Id { kind: Kind::Backend, name: i.id.0.to_owned() },
            from: self.backend_from(i.id.symbol()).cloned().unwrap_or(Provider::Linked),
            readiness: i.readiness,
            handles: i.arches,
            outputs: vec![i.caps.emits.name()],
        });
        let linkers = self.linkers().map(vun_link::Linker::info).map(|i| Entry {
            id: Id { kind: Kind::Linker, name: i.id.0.to_owned() },
            from: self.linker_from(i.id.symbol()).cloned().unwrap_or(Provider::Linked),
            readiness: i.readiness,
            handles: Names::Only(i.formats),
            outputs: i.outputs.iter().map(|o| o.name()).collect(),
        });
        let plain = |kind, name: &str, from: &Provider| Entry::plain(kind, name, from);
        let schemes = self.schemes.registered().map(|c| plain(Kind::Scheme, c.item.name(), &c.from));
        let servers = self.servers.registered().map(|c| plain(Kind::Server, c.item.name(), &c.from));
        let binders = self.binders.registered().map(|c| plain(Kind::Binder, c.item.name(), &c.from));
        let formatters = self.formatters.registered().map(|c| plain(Kind::Formatter, c.item.name(), &c.from));
        let documenters =
            self.documenters.registered().map(|c| plain(Kind::Documenter, c.item.name(), &c.from));
        let renderers = self.renderers.registered().map(|c| plain(Kind::Renderer, c.item.name(), &c.from));
        backends
            .chain(linkers)
            .chain(schemes)
            .chain(servers)
            .chain(binders)
            .chain(formatters)
            .chain(documenters)
            .chain(renderers)
            .collect()
    }
}
