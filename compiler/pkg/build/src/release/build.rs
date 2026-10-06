//! What a build of a release holds: its components, which profile holds each, and what each
//! needs.

use vun_target::Machine;

use super::{Build, Piece, Profile};

impl Build {
    /// The suffix of toolchain archives: gzip-compressed tar, which every system unpacks
    /// without `vun`, as a first installation does.
    pub const SUFFIX: &str = ".tar.gz";

    /// The components `profile` holds.
    pub fn in_profile(&self, profile: Profile) -> impl Iterator<Item = &Piece> {
        self.components.iter().filter(move |piece| piece.profile <= profile)
    }

    /// The component `name` of this build.
    #[must_use]
    pub fn piece(&self, name: &str) -> Option<&Piece> {
        self.components.iter().find(|p| p.name == name)
    }

    /// The components `names` and what they need, each once, those needed before those needing
    /// them.
    ///
    /// # Errors
    /// Returns the first name that is not a component of this build.
    pub fn with_needs<'a>(&'a self, names: &'a [String]) -> Result<Vec<&'a Piece>, &'a str> {
        let (mut seen, mut found) = (Vec::new(), Vec::new());
        for name in names {
            self.visit(name, &mut seen, &mut found)?;
        }
        Ok(found)
    }

    fn visit<'a>(
        &'a self,
        name: &'a str,
        seen: &mut Vec<&'a str>,
        found: &mut Vec<&'a Piece>,
    ) -> Result<(), &'a str> {
        if seen.contains(&name) {
            return Ok(());
        }
        seen.push(name);
        let piece = self.piece(name).ok_or(name)?;
        for need in &piece.needs {
            self.visit(need, seen, found)?;
        }
        found.push(piece);
        Ok(())
    }

    /// The needs of its components that no component of this build meets in the same or a
    /// smaller profile: what a toolchain installed from it would lack.
    pub fn unmet(&self) -> impl Iterator<Item = (&Piece, &str)> {
        self.components.iter().flat_map(move |piece| {
            let unmet = move |need: &&String| self.piece(need).is_none_or(|p| p.profile > piece.profile);
            piece.needs.iter().filter(unmet).map(move |need| (piece, need.as_str()))
        })
    }

    /// The file name of the archive of `machine`'s toolchain: the same in every release, so that
    /// the latest one is found without its version (`vun-x86_64-linux.tar.gz`).
    #[must_use]
    pub fn archive_name(machine: Machine) -> String {
        [crate::toolchains::NAME, "-", machine.arch.0, "-", machine.os.0, Self::SUFFIX].concat()
    }
}

impl Piece {
    /// The file name of the archive of the component `name` for `machine`
    /// (`vun-cranelift-x86_64-linux.tar.gz`).
    #[must_use]
    pub fn archive_name(name: &str, machine: Machine) -> String {
        [crate::toolchains::NAME, "-", name, "-", machine.arch.0, "-", machine.os.0, Build::SUFFIX].concat()
    }
}
