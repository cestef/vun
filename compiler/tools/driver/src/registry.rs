use vun_backend::Backend;
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
    pub fn with_defaults() -> Self {
        let mut r = Self::default();
        register!(r.add_backend {
            "backend-c" => vun_backend_c::CBackend,
            "backend-cranelift" => vun_backend_cranelift::CraneliftBackend,
            "backend-qbe" => vun_backend_qbe::QbeBackend,
            "backend-llvm" => vun_backend_llvm::LlvmBackend,
            "backend-process" => vun_backend_process::ProcessBackend,
        });
        register!(r.add_linker {
            "link-wild" => vun_link_wild::WildLinker,
            "link-mold" => vun_link_mold::MoldLinker,
            "link-lld" => vun_link_lld::LldLinker,
            "link-gnu" => vun_link_gnu::GnuLinker,
            "link-msvc" => vun_link_msvc::MsvcLinker,
            "link-process" => vun_link_process::ProcessLinker,
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
    pub fn backend(&self, id: &str) -> Option<&dyn Backend> {
        self.backends().find(|b| b.info().id == id)
    }

    #[must_use]
    pub fn linker(&self, id: &str) -> Option<&dyn Linker> {
        self.linkers().find(|l| l.info().id == id)
    }
}

#[cfg(test)]
mod tests;
