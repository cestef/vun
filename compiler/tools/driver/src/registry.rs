mod choose;

pub use choose::{Codegen, Unchosen, Unfit};

use vun_backend::Backend;
use vun_intern::Symbol;
use vun_link::{Linker, Output};
use vun_target::TargetSpec;

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

    /// The adapter building a program as `output` for `target`: the target's linker where it
    /// makes that output, else the first that makes it for the target's object format (the
    /// archiver, for a static library).
    #[must_use]
    pub fn linker_for(&self, output: Output, target: &TargetSpec) -> Symbol {
        let makes = |l: &&dyn Linker| {
            l.info().outputs.contains(&output) && l.info().formats.iter().any(|f| f.is(target.object_format))
        };
        match self.linker(target.linker) {
            Some(own) if makes(&own) => target.linker,
            _ => self.linkers().find(makes).map_or(target.linker, |l| l.info().id.symbol()),
        }
    }
}

#[cfg(test)]
mod tests;
