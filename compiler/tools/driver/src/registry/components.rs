//! The components of the toolchain's `components` directory (decision 498): libraries each
//! serving a backend, a linker, a URL scheme, a server, a binder or a renderer, registered beside those linked in.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};

use vun_backend_component::Backends;
use vun_binder::Binders;
use vun_build::release::{Piece, Release};
use vun_build::toolchains::{COMPONENTS, Toolchains};
use vun_component::{Component, Fault, Interface, Kind};
use vun_data::FromData;
use vun_documenter::Documenters;
use vun_formatter::Formatters;
use vun_link_component::Linkers;
use vun_renderer::Renderers;
use vun_scheme_component::Schemes;
use vun_server::Servers;
use vun_target::Machine;
use vun_trace::tracing::{debug, warn};

use super::{Provider, Registry};

/// What opens a component of a kind into what the registry takes.
type Open = fn(Component) -> Result<Served, Fault>;

/// What a library serves, opened once for the process.
#[derive(Clone)]
enum Served {
    Backend(vun_backend_component::Remote),
    Linker(vun_link_component::Remote),
    Scheme(vun_scheme_component::Remote),
    Server(vun_server::Remote),
    Binder(vun_binder::Remote),
    Formatter(vun_formatter::Remote),
    Documenter(vun_documenter::Remote),
    Renderer(vun_renderer::Remote),
}

impl Served {
    /// The kinds of component there are: each with what opens a component of it.
    const KINDS: &[(Kind, Open)] = &[
        (Backends::KIND, |c| vun_backend_component::Remote::of(c).map(Self::Backend)),
        (Linkers::KIND, |c| vun_link_component::Remote::of(c).map(Self::Linker)),
        (Schemes::KIND, |c| vun_scheme_component::Remote::of(c).map(Self::Scheme)),
        (Servers::KIND, |c| vun_server::Remote::of(c).map(Self::Server)),
        (Binders::KIND, |c| vun_binder::Remote::of(c).map(Self::Binder)),
        (Formatters::KIND, |c| vun_formatter::Remote::of(c).map(Self::Formatter)),
        (Documenters::KIND, |c| vun_documenter::Remote::of(c).map(Self::Documenter)),
        (Renderers::KIND, |c| vun_renderer::Remote::of(c).map(Self::Renderer)),
    ];

    /// What the component is called: its kind and its backend's or linker's id
    /// (`backend-cranelift`).
    fn name(&self) -> String {
        match self {
            Self::Backend(backend) => {
                format!("{}-{}", Backends::KIND, vun_backend::Backend::info(backend).id)
            }
            Self::Linker(linker) => format!("{}-{}", Linkers::KIND, vun_link::Linker::info(linker).id),
            Self::Renderer(renderer) => {
                format!("{}-{}", Renderers::KIND, vun_renderer::Renderer::name(renderer))
            }
            Self::Binder(binder) => format!("{}-{}", Binders::KIND, vun_binder::Binder::name(binder)),
            Self::Formatter(formatter) => {
                format!("{}-{}", Formatters::KIND, vun_formatter::Formatter::name(formatter))
            }
            Self::Documenter(documenter) => {
                format!("{}-{}", Documenters::KIND, vun_documenter::Documenter::name(documenter))
            }
            Self::Server(server) => format!("{}-{}", Servers::KIND, vun_server::Server::name(server)),
            Self::Scheme(scheme) => format!("{}-{}", Schemes::KIND, vun_fetch::Scheme::name(scheme)),
        }
    }

    /// What the library at `path` serves, opened the first time and kept.
    fn of(path: &Path) -> Result<Self, Fault> {
        static OPENED: LazyLock<Mutex<HashMap<PathBuf, Served>>> = LazyLock::new(Mutex::default);
        let mut opened = OPENED.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(served) = opened.get(path) {
            return Ok(served.clone());
        }
        let component = Component::open(path)?;
        let (_, open) = Self::KINDS
            .iter()
            .find(|(kind, _)| kind.0 == component.kind())
            .ok_or_else(|| Fault::Unserved(component.kind().to_owned()))?;
        let served = open(component)?;
        opened.insert(path.to_owned(), served.clone());
        Ok(served)
    }
}

/// The components of a toolchain: the directory of their libraries, and, for a toolchain
/// installed from a release, what the release's manifest says each is, which a library must be
/// to be loaded.
pub struct Components {
    dir: PathBuf,
    /// The toolchain's tree, and the pieces its kept manifest names for this machine.
    release: Option<(PathBuf, Vec<Piece>)>,
}

impl Components {
    /// The environment variable naming the directory, instead of the toolchain's.
    pub const ENV: &str = "VUN_COMPONENTS";

    /// The libraries of `dir`, whatever they are.
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into(), release: None }
    }

    /// The running toolchain's, if it has a directory of them.
    #[must_use]
    pub fn find() -> Option<Self> {
        if let Some(dir) = std::env::var_os(Self::ENV) {
            return Some(Self::at(dir));
        }
        let exe = vun_fs::EXECUTABLE.as_deref()?;
        let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
        let root = exe.parent()?.parent()?.to_path_buf();
        let dir = root.join(COMPONENTS);
        let pieces = Self::pieces(&root);
        dir.is_dir().then(|| Self { dir, release: pieces.map(|pieces| (root, pieces)) })
    }

    /// What the manifest kept in the toolchain tree `root` names for this machine.
    fn pieces(root: &Path) -> Option<Vec<Piece>> {
        let text = std::fs::read_to_string(root.join(Release::FILE)).ok()?;
        let release = Release::from_data(&vun_data::json::parse(&text).ok()?).ok()?;
        release.build_for(Machine::RUNNING).map(|build| build.components.clone())
    }

    /// The name the component of the library at `path` goes by.
    ///
    /// # Errors
    /// Returns why it is not a component this toolchain can open.
    pub fn named(path: &Path) -> Result<String, Fault> {
        Served::of(path).map(|served| served.name())
    }

    /// Whether the library at `path` is the one the release names, if one does.
    fn admits(&self, path: &Path) -> bool {
        let Some((root, pieces)) = &self.release else { return true };
        let file = path.file_name().and_then(|f| f.to_str());
        let Some(piece) = pieces.iter().find(|p| Some(p.file.as_str()) == file) else { return false };
        Toolchains::hash_of(root, &[&Toolchains::library(piece)]).is_ok_and(|hash| hash == piece.hash)
    }
}

impl Registry {
    /// Registers what the libraries of `components` serve, those not already registered: a file
    /// that is not a component is passed over, a library the release does not name as it is, or
    /// of another release, is refused.
    pub fn load(&mut self, components: &Components) {
        let Ok(entries) = std::fs::read_dir(&components.dir) else { return };
        let mut libraries: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|e| e == std::env::consts::DLL_EXTENSION))
            .collect();
        libraries.sort();
        for path in libraries {
            if !components.admits(&path) {
                warn!(library = %path.display(), "a library is not the one the release names");
                continue;
            }
            match Served::of(&path) {
                Ok(Served::Backend(backend)) => {
                    self.add_backend_from(Box::new(backend), Provider::Loaded(path));
                }
                Ok(Served::Linker(linker)) => {
                    self.add_linker_from(Box::new(linker), Provider::Loaded(path));
                }
                Ok(Served::Scheme(scheme)) => {
                    self.schemes.load(Box::new(scheme), path);
                }
                Ok(Served::Server(server)) => {
                    self.servers.load(Box::new(server), path);
                }
                Ok(Served::Binder(binder)) => {
                    self.binders.load(Box::new(binder), path);
                }
                Ok(Served::Formatter(formatter)) => {
                    self.formatters.load(Box::new(formatter), path);
                }
                Ok(Served::Documenter(documenter)) => {
                    self.documenters.load(Box::new(documenter), path);
                }
                Ok(Served::Renderer(renderer)) => {
                    self.renderers.load(Box::new(renderer), path);
                }
                Err(Fault::Open { source, .. }) => {
                    debug!(library = %path.display(), %source, "not a component");
                }
                Err(fault) => warn!(library = %path.display(), %fault, "a component is refused"),
            }
        }
    }
}
