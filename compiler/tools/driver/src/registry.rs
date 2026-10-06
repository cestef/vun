mod choose;

pub use choose::{Codegen, Link, Linkage, Pick, Picked, Unchosen, Unfit};

use vun_backend::Backend;
use vun_intern::Symbol;
use vun_link::Linker;

/// The backends and linkers available to a session.
#[derive(Default)]
pub struct Registry {
    backends: Vec<Box<dyn Backend>>,
    linkers: Vec<Box<dyn Linker>>,
}

macro_rules! register {
    ($registry:ident.$method:ident { $($feature:literal => $component:expr),* $(,)? }) => {
        $(
            #[cfg(feature = $feature)]
            $registry.$method(Box::new($component));
        )*
    };
}

impl Registry {
    /// Every backend and linker compiled into this toolchain.
    #[must_use]
    #[allow(unused_mut, reason = "a toolchain built without features registers nothing")]
    pub fn with_defaults() -> Self {
        let mut r = Self::default();
        register!(r.add_backend {
            "backend-c" => vun_backend_c::CBackend::default(),
            "backend-native" => vun_backend_native::NativeBackend,
            "backend-wasm" => vun_backend_wasm::WasmBackend,
            "backend-cranelift" => vun_backend_cranelift::BACKEND,
            "backend-qbe" => vun_backend_qbe::BACKEND,
            "backend-llvm" => vun_backend_llvm::BACKEND,
            "backend-external" => vun_backend_external::BACKEND,
        });
        register!(r.add_linker {
            "link-wild" => vun_link_wild::WildLinker,
            "link-mold" => vun_link_mold::MoldLinker,
            "link-ar" => vun_link_ar::ArLinker,
            "link-lld" => vun_link_lld::LldLinker,
            "link-wasm" => vun_link_wasm::WasmLinker,
            "link-gnu" => vun_link_gnu::GnuLinker,
            "link-msvc" => vun_link_msvc::MsvcLinker,
            "link-external" => vun_link_external::ExternalLinker,
        });
        r
    }

    pub fn add_backend(&mut self, backend: Box<dyn Backend>) {
        self.backends.push(backend);
    }

    pub fn add_linker(&mut self, linker: Box<dyn Linker>) {
        self.linkers.push(linker);
    }

    pub fn backends(&self) -> impl Iterator<Item = &dyn Backend> {
        self.backends.iter().map(AsRef::as_ref)
    }

    pub fn linkers(&self) -> impl Iterator<Item = &dyn Linker> {
        self.linkers.iter().map(AsRef::as_ref)
    }

    #[must_use]
    pub fn backend(&self, name: Symbol) -> Option<&dyn Backend> {
        self.backends().find(|b| b.info().id.is(name))
    }

    #[must_use]
    pub fn linker(&self, name: Symbol) -> Option<&dyn Linker> {
        self.linkers().find(|l| l.info().id.is(name))
    }
}

#[cfg(test)]
mod tests;
