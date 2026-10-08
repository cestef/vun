mod assist;
mod choose;
#[cfg(feature = "components")]
mod components;
mod entries;

pub use choose::{Codegen, Link, Linkage, Pick, Picked, Unchosen, Unfit};
#[cfg(feature = "components")]
pub use components::Components;
pub(crate) use entries::Entry;
pub use entries::{Id, Kind};

use std::path::PathBuf;

use vun_backend::Backend;
use vun_binder::Binder;
use vun_documenter::Documenter;
use vun_fetch::{Scheme, Schemes};
use vun_formatter::Formatter;
use vun_intern::Symbol;
use vun_link::Linker;
use vun_renderer::{Renderer, Short};
use vun_server::Server;

mod roster;

pub use roster::{Named, Roster};

/// The components available to a session: backends, linkers, URL schemes, servers, binders, formatters, documenters and renderers.
#[derive(Default)]
pub struct Registry {
    backends: Roster<dyn Backend>,
    linkers: Roster<dyn Linker>,
    pub schemes: Roster<dyn Scheme>,
    pub servers: Roster<dyn Server>,
    pub binders: Roster<dyn Binder>,
    pub formatters: Roster<dyn Formatter>,
    pub documenters: Roster<dyn Documenter>,
    pub renderers: Roster<dyn Renderer>,
}

/// Where a component comes from (decision 498).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Compiled into `vun`.
    Linked,
    /// Loaded from this library of the toolchain's `components` directory.
    Loaded(PathBuf),
}

impl Named for dyn Backend {
    fn called(&self) -> &str {
        self.info().id.0
    }
}

impl Named for dyn Linker {
    fn called(&self) -> &str {
        self.info().id.0
    }
}

impl Named for dyn Renderer {
    fn called(&self) -> &str {
        self.name()
    }
}

impl Named for dyn Binder {
    fn called(&self) -> &str {
        self.name()
    }
}

impl Named for dyn Documenter {
    fn called(&self) -> &str {
        self.name()
    }
}

impl Named for dyn Formatter {
    fn called(&self) -> &str {
        self.name()
    }
}

impl Named for dyn Server {
    fn called(&self) -> &str {
        self.name()
    }
}

impl Named for dyn Scheme {
    fn called(&self) -> &str {
        self.name()
    }
}

macro_rules! register {
    ($recv:expr, $method:ident { $($feature:literal => $component:expr),* $(,)? }) => {
        $(
            #[cfg(feature = $feature)]
            $recv.$method(Box::new($component));
        )*
    };
}

impl Registry {
    /// Every backend and linker compiled into this toolchain.
    #[must_use]
    #[allow(unused_mut, reason = "a toolchain built without features registers nothing")]
    pub fn with_defaults() -> Self {
        let mut r = Self::default();
        register!(r, add_backend {
            "backend-c" => vun_component_backend_c::component(),
            "backend-native" => vun_component_backend_native::component(),
            "backend-wasm" => vun_component_backend_wasm::component(),
            "backend-cranelift" => vun_component_backend_cranelift::component(),
            "backend-qbe" => vun_backend_qbe::BACKEND,
            "backend-llvm" => vun_backend_llvm::BACKEND,
            "backend-external" => vun_backend_external::BACKEND,
        });
        register!(r, add_linker {
            "link-wild" => vun_component_link_wild::component(),
            "link-mold" => vun_component_link_mold::component(),
            "link-ar" => vun_component_link_ar::component(),
            "link-lld" => vun_component_link_lld::component(),
            "link-wasm" => vun_component_link_wasm::component(),
            "link-gnu" => vun_component_link_gnu::component(),
            "link-msvc" => vun_link_msvc::MsvcLinker,
            "link-external" => vun_link_external::ExternalLinker,
        });
        register!(r.schemes, link {
            "scheme-https" => vun_component_scheme_https::component(),
        });
        r.renderers.link(Box::new(Short));
        register!(r.renderers, link {
            "renderer-terminal" => vun_component_renderer_terminal::component(),
            "renderer-ariadne" => vun_component_renderer_ariadne::component(),
            "renderer-sarif" => vun_component_renderer_sarif::component(),
        });
        #[cfg(feature = "components")]
        if let Some(components) = Components::find() {
            r.load(&components);
        }
        r
    }

    /// Registers `backend`, linked in, unless one of its name is registered.
    pub fn add_backend(&mut self, backend: Box<dyn Backend>) {
        self.add_backend_from(backend, Provider::Linked);
    }

    /// Registers `backend` as `from` provides it, unless one of its name is: the first registered,
    /// linked in or loaded, is the one a build uses.
    pub fn add_backend_from(&mut self, backend: Box<dyn Backend>, from: Provider) {
        self.backends.add(backend.into(), from);
    }

    /// Registers `linker`, linked in, as [`Registry::add_backend`] does a backend.
    pub fn add_linker(&mut self, linker: Box<dyn Linker>) {
        self.add_linker_from(linker, Provider::Linked);
    }

    /// Registers `linker` as `from` provides it, as [`Registry::add_backend_from`] does a backend.
    pub fn add_linker_from(&mut self, linker: Box<dyn Linker>, from: Provider) {
        self.linkers.add(linker.into(), from);
    }

    /// The renderer called `name`, or the default format's, or the one line that is always there.
    #[must_use]
    pub fn renderer(&self, name: Symbol) -> &dyn Renderer {
        let found = self.renderers.get(name.as_str()).or_else(|| self.renderers.get(crate::formats::DEFAULT));
        found.map_or(&Short, |r| &*r.item)
    }

    pub fn backends(&self) -> impl Iterator<Item = &dyn Backend> {
        self.backends.iter().map(|b| b as &dyn Backend)
    }

    pub fn linkers(&self) -> impl Iterator<Item = &dyn Linker> {
        self.linkers.iter().map(|l| l as &dyn Linker)
    }

    #[must_use]
    pub fn backend(&self, name: Symbol) -> Option<&dyn Backend> {
        self.backends.get(name.as_str()).map(|r| &*r.item)
    }

    #[must_use]
    pub fn linker(&self, name: Symbol) -> Option<&dyn Linker> {
        self.linkers.get(name.as_str()).map(|r| &*r.item)
    }

    /// The URL schemes packages and toolchains are read through: `file`, and those registered.
    #[must_use]
    pub fn schemes(&self) -> Schemes {
        let mut schemes = Schemes::default();
        for registered in self.schemes.registered() {
            schemes.install(registered.item.clone());
        }
        schemes
    }

    /// Where the backend `name` comes from.
    #[must_use]
    pub fn backend_from(&self, name: Symbol) -> Option<&Provider> {
        self.backends.get(name.as_str()).map(|r| &r.from)
    }

    /// Where the linker `name` comes from.
    #[must_use]
    pub fn linker_from(&self, name: Symbol) -> Option<&Provider> {
        self.linkers.get(name.as_str()).map(|r| &r.from)
    }
}

#[cfg(test)]
mod tests;
