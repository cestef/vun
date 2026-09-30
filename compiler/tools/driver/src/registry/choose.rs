//! Choosing the backend of a build (spec 13, section 8): the one asked for by name, else the first
//! of the target's preferences for the profile that this toolchain has and that generates code
//! for the target; and the linker. What is chosen says how, and why no backend is chosen is
//! typed, and becomes a diagnostic in the driver.

use vun_backend::{Backend, Readiness};
use vun_intern::Symbol;
use vun_link::Linker;
use vun_target::{Name, Profile, Support, TargetSpec, Unsupported};

use super::Registry;

/// Which backend a build asks for, which linker, and what it builds the program as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Codegen {
    pub profile: Profile,
    /// The backend named (`--backend`), overriding the target's preferences.
    pub backend: Option<Symbol>,
    /// The linker named (`--linker`), overriding the target's.
    pub linker: Option<Symbol>,
    /// An executable, or the library `--library` or the plan names.
    pub output: vun_link::Output,
}

impl Codegen {
    /// A debug build through the backend `name`.
    #[must_use]
    pub fn through(name: Name) -> Self {
        Self { backend: Some(name.symbol()), ..Self::default() }
    }
}

/// Why a backend cannot build for a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// This toolchain has no backend of that name.
    Absent,
    /// It is registered but generates no code yet.
    Planned,
    /// It does not generate code for the target's architecture or object format.
    Unsupported(Unsupported),
}

/// Why no backend is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchosen {
    /// The backend named cannot build for the target.
    Named(Symbol, Unfit),
    /// None of the target's preferences for the profile can, each for its reason.
    Preferred(Profile, Vec<(Symbol, Unfit)>),
}

/// The backend chosen, and how.
pub struct Picked<'r> {
    pub backend: &'r dyn Backend,
    pub by: Pick,
}

/// How a backend is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Named by `--backend`.
    Named,
    /// The first of the target's preferences for the profile that can build, after those that
    /// cannot, each for its reason.
    Preferred(Profile, Vec<(Symbol, Unfit)>),
}

/// The linker a program is linked with, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Linkage {
    pub linker: Symbol,
    pub by: Link,
}

/// Why a linker is the one a program is linked with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// Named by `--linker`.
    Named,
    /// The target's own.
    Target,
    /// The first that makes the output for the target's object format, the target's own not
    /// making it (the archiver, for a static library).
    Instead(Symbol),
}

impl Registry {
    /// The backend building for `target` as `codegen` asks.
    ///
    /// # Errors
    /// Returns why none can.
    pub fn choose(&self, codegen: Codegen, target: &TargetSpec) -> Result<Picked<'_>, Unchosen> {
        if let Some(name) = codegen.backend {
            let backend = self.fit(name, target).map_err(|unfit| Unchosen::Named(name, unfit))?;
            return Ok(Picked { backend, by: Pick::Named });
        }
        let preferred = target.backends.of(codegen.profile);
        let mut passed = Vec::with_capacity(preferred.len());
        for &name in preferred {
            match self.fit(name, target) {
                Ok(backend) => return Ok(Picked { backend, by: Pick::Preferred(codegen.profile, passed) }),
                Err(unfit) => passed.push((name, unfit)),
            }
        }
        Err(Unchosen::Preferred(codegen.profile, passed))
    }

    /// The backend `name`, if it builds for `target`.
    fn fit(&self, name: Symbol, target: &TargetSpec) -> Result<&dyn Backend, Unfit> {
        let backend = self.backend(name).ok_or(Unfit::Absent)?;
        let info = backend.info();
        if info.readiness == Readiness::Planned {
            return Err(Unfit::Planned);
        }
        match info.supports(target) {
            Support::Yes => Ok(backend),
            Support::No(why) => Err(Unfit::Unsupported(why)),
        }
    }

    /// The backends that build for `target`, in the registry's order.
    pub fn fitting<'a>(&'a self, target: &'a TargetSpec) -> impl Iterator<Item = &'a dyn Backend> {
        self.backends().filter(|b| self.fit(b.info().id.symbol(), target).is_ok())
    }

    /// The linker building a program as `codegen` asks for `target`: the one named, else the
    /// target's where it makes that output, else the first that makes it for the target's object
    /// format.
    #[must_use]
    pub fn linkage(&self, codegen: Codegen, target: &TargetSpec) -> Linkage {
        if let Some(linker) = codegen.linker {
            return Linkage { linker, by: Link::Named };
        }
        let makes = |l: &&dyn Linker| {
            l.info().outputs.contains(&codegen.output)
                && l.info().formats.iter().any(|f| f.is(target.object_format))
        };
        match self.linker(target.linker) {
            Some(own) if makes(&own) => Linkage { linker: target.linker, by: Link::Target },
            _ => match self.linkers().find(makes) {
                Some(other) => Linkage { linker: other.info().id.symbol(), by: Link::Instead(target.linker) },
                None => Linkage { linker: target.linker, by: Link::Target },
            },
        }
    }
}
