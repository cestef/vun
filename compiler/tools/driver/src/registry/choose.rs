//! Choosing the backend of a build (spec 13, section 8): the one asked for by name, else the first
//! of the target's preferences for the profile that this toolchain has and that generates code
//! for the target. Why no backend is chosen is typed, and becomes a diagnostic in the driver.

use vun_backend::{Backend, Readiness};
use vun_intern::Symbol;
use vun_target::{Name, Profile, Support, TargetSpec};

use super::Registry;

/// Which backend a build asks for, and which linker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Codegen {
    pub profile: Profile,
    /// The backend named (`--backend`), overriding the target's preferences.
    pub backend: Option<Symbol>,
    /// The linker named (`--linker`), overriding the target's.
    pub linker: Option<Symbol>,
}

impl Codegen {
    /// A debug build through the backend `name`.
    #[must_use]
    pub fn through(name: Name) -> Self {
        Self { profile: Profile::Debug, backend: Some(name.symbol()), linker: None }
    }
}

/// Why a backend cannot build for a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// This toolchain has no backend of that name.
    Absent,
    /// It is registered but generates no code yet.
    Planned,
    /// It does not generate code for the target's architecture.
    Arch,
}

/// Why no backend is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unchosen {
    /// The backend named cannot build for the target.
    Named(Symbol, Unfit),
    /// None of the target's preferences for the profile can, each for its reason.
    Preferred(Profile, Vec<(Symbol, Unfit)>),
}

impl Registry {
    /// The backend building for `target` as `codegen` asks.
    ///
    /// # Errors
    /// Returns why none can.
    pub fn choose(&self, codegen: Codegen, target: &TargetSpec) -> Result<&dyn Backend, Unchosen> {
        if let Some(name) = codegen.backend {
            return self.fit(name, target).map_err(|unfit| Unchosen::Named(name, unfit));
        }
        let preferred = target.backends.of(codegen.profile);
        let mut tried = Vec::with_capacity(preferred.len());
        for &name in preferred {
            match self.fit(name, target) {
                Ok(backend) => return Ok(backend),
                Err(unfit) => tried.push((name, unfit)),
            }
        }
        Err(Unchosen::Preferred(codegen.profile, tried))
    }

    /// The backend `name`, if it builds for `target`.
    fn fit(&self, name: Symbol, target: &TargetSpec) -> Result<&dyn Backend, Unfit> {
        let backend = self.backend(name).ok_or(Unfit::Absent)?;
        let info = backend.info();
        if info.readiness == Readiness::Planned {
            return Err(Unfit::Planned);
        }
        if info.supports(target) != Support::Yes {
            return Err(Unfit::Arch);
        }
        Ok(backend)
    }

    /// The backends that build for `target`, in the registry's order.
    pub fn fitting<'a>(&'a self, target: &'a TargetSpec) -> impl Iterator<Item = &'a dyn Backend> {
        self.backends().filter(|b| self.fit(b.info().id.symbol(), target).is_ok())
    }
}
