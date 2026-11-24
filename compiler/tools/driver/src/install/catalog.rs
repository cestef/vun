//! What a toolchain has of its components (spec 12, section 6): those built into its executable,
//! those loaded from its `components` directory, and those its release builds that are
//! installed or can be, in one list.

use std::path::PathBuf;

use vun_build::release::{Build, Profile};
use vun_build::toolchains::Toolchains;
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_span::Version;
use vun_suggest::closest;
use vun_target::{Machine, Names, Readiness};

use super::Held;
use crate::registry::{Entry, Id, Kind};
use crate::{Provider, Session, VERSION};

/// How a toolchain has a component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Has {
    /// Compiled into the executable.
    BuiltIn,
    /// Loaded from this library of the toolchain's `components` directory.
    Loaded(PathBuf),
    /// Installed from the release, to load when asked for.
    Installed,
    /// Built by the release, not installed.
    Available,
}

/// A component of a toolchain.
#[derive(Clone, Debug)]
pub struct Component {
    /// `None` for a piece of a release named for a kind this toolchain does not know.
    pub kind: Option<Kind>,
    pub name: String,
    pub has: Has,
    pub readiness: Readiness,
    pub handles: Names,
    pub outputs: Vec<&'static str>,
    /// The smallest profile of the release that holds it, when the release builds it.
    pub profile: Option<Profile>,
}

impl Component {
    fn entry(entry: Entry) -> Self {
        let has = match entry.from {
            Provider::Linked => Has::BuiltIn,
            Provider::Loaded(path) => Has::Loaded(path),
        };
        Self {
            kind: Some(entry.id.kind),
            name: entry.id.name,
            has,
            readiness: entry.readiness,
            handles: entry.handles,
            outputs: entry.outputs,
            profile: None,
        }
    }

    fn piece(held: &Held<'_>) -> Self {
        let id = held.piece.name.parse::<Id>().ok();
        Self {
            kind: id.as_ref().map(|id| id.kind),
            name: id.map_or_else(|| held.piece.name.clone(), |id| id.name),
            has: if held.installed { Has::Installed } else { Has::Available },
            readiness: Readiness::Ready,
            handles: Names::Any,
            outputs: Vec::new(),
            profile: Some(held.piece.profile),
        }
    }
}

impl Session {
    /// The components of the toolchain of `version`: the running toolchain's own, linked or
    /// loaded, then those of its release that it does not have already.
    #[must_use]
    pub fn components_of(&self, toolchains: Option<&Toolchains>, version: Version) -> Vec<Component> {
        let running = VERSION.parse::<Version>().ok() == Some(version);
        let mut listed: Vec<Component> = if running {
            self.registry.entries().into_iter().map(Component::entry).collect()
        } else {
            Vec::new()
        };
        let release = toolchains.and_then(|t| t.release(version));
        let build = release.as_ref().and_then(|r| r.build_for(Machine::RUNNING));
        let held = toolchains.zip(build).map(|(t, build)| Held::of(t, version, build)).unwrap_or_default();
        for held in held {
            let id = held.piece.name.parse::<Id>().ok();
            let same = |c: &&mut Component| {
                id.as_ref().is_some_and(|id| c.kind == Some(id.kind) && c.name == id.name)
            };
            match listed.iter_mut().find(same) {
                Some(row) => row.profile = Some(held.piece.profile),
                None => listed.push(Component::piece(&held)),
            }
        }
        listed
    }

    /// The pieces of `build` the words name, each by its full name or by the name its interface
    /// gives it when only one has that; `None` once what names none, or several, is reported.
    pub fn pieces_named(&mut self, build: &Build, words: &[&str]) -> Option<Vec<String>> {
        let mut named = Vec::new();
        let mut found = true;
        for word in words {
            let by_name = |name: &str| name.parse::<Id>().is_ok_and(|id| id.name == *word);
            let exact = build.piece(word);
            let bare: Vec<&str> =
                build.components.iter().map(|p| p.name.as_str()).filter(|n| by_name(n)).collect();
            match (exact, bare.as_slice()) {
                (Some(piece), _) => named.push(piece.name.clone()),
                (None, [only]) => named.push((*only).to_owned()),
                (None, []) => {
                    let known = || build.components.iter().map(|p| p.name.as_str());
                    let near = closest(word, known());
                    let msg = Msg::new().text("the release builds no component ").code(*word);
                    let help = near.map_or_else(
                        || Msg::new().text("the release builds ").code_list(known()),
                        Msg::did_you_mean,
                    );
                    self.diag.error(Diagnostic::error(errors::TOOLCHAIN_NO_COMPONENT, msg).help(help));
                    found = false;
                }
                (None, several) => {
                    let msg = Msg::new().code(*word).text(" names more than one component");
                    let help = Msg::new().text("name one in full: ").code_list(several.iter().copied());
                    self.diag.error(Diagnostic::error(errors::TOOLCHAIN_NO_COMPONENT, msg).help(help));
                    found = false;
                }
            }
        }
        found.then_some(named)
    }

    /// Removes the libraries of the pieces `names` of `build` from the toolchain of `version`,
    /// unless another piece installed needs one; reported then, and nothing is removed. Gives
    /// each piece with whether it was installed.
    ///
    /// # Errors
    /// Returns once a piece kept by another is reported, or a library cannot be removed.
    pub fn uninstall_components(
        &mut self,
        toolchains: &Toolchains,
        version: Version,
        build: &Build,
        names: &[String],
    ) -> Result<Vec<(String, bool)>, ErrorGuaranteed> {
        let held = Held::of(toolchains, version, build);
        let mut needed = None;
        for needing in held.iter().filter(|h| h.installed && !names.contains(&h.piece.name)) {
            for name in names.iter().filter(|n| needing.piece.needs.contains(n)) {
                let msg = Msg::new().code(name).text(" is needed by ").code(&needing.piece.name);
                needed = Some(self.diag.error(Diagnostic::error(errors::TOOLCHAIN_REQUIRED, msg)));
            }
        }
        if let Some(reported) = needed {
            return Err(reported);
        }
        let mut removed = Vec::new();
        for piece in names.iter().filter_map(|name| build.piece(name)) {
            let was = held.iter().any(|h| h.installed && h.piece.name == piece.name);
            toolchains.remove_component(version, piece).map_err(|e| {
                let library = toolchains.components(version).join(&piece.file);
                self.diag.error(Diagnostic::unwritable(&library, &e))
            })?;
            removed.push((piece.name.clone(), was));
        }
        Ok(removed)
    }
}
