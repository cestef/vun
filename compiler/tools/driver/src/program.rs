//! Building a program from a root file (spec 13, section 1): the chosen backend's artifacts,
//! written under `.vun/build/` next to the root file, by their content; those that are source compiled by the target's
//! C compiler; then the target's linker.

use std::path::{Path, PathBuf};

use vun_backend::{Artifact, Emits};
use vun_command::{Param, ParamKind};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Edit;
use vun_hash::Hash;
use vun_link::{LinkPlan, OutputKind};
use vun_span::{FileId, SourceMap};
use vun_tool::{Executable, Invocation, Tool};

use crate::codegen::Uncoded;
use crate::{Registry, Session};

pub const OUTPUT: Param = Param {
    name: "output",
    short: Some('o'),
    about: "where the program is written",
    kind: ParamKind::Option,
};

/// Where builds keep their files, under the root file's directory (spec 12, section 1).
const BUILDS: &str = ".vun/build";

/// The extension of an object file.
const OBJECT: &str = "o";

impl Session {
    /// Builds the program rooted at `file` into `output`, or next to the builds, named as the
    /// file.
    ///
    /// # Errors
    /// Returns once what stopped the build is reported.
    ///
    /// # Panics
    /// Never: naming a file chooses the target.
    pub fn build(
        &mut self,
        file: FileId,
        backend: Option<&str>,
        output: Option<&Path>,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        let artifacts = self.code(file, backend).map_err(|uncoded| match uncoded {
            Uncoded::Reported(guar) => guar,
            Uncoded::Unlowered(unlowered) => {
                let msg = Msg::new().text("the program does not lower to LIR: ").text(unlowered.to_string());
                self.diag.error(Diagnostic::error(errors::BACKEND_UNLOWERED, msg).at(unlowered.at, None))
            }
        })?;
        let target = self.target.clone().expect("naming a file chooses the target")?;
        let root = PathBuf::from(&self.sources.file(file).path);
        let builds = root.parent().unwrap_or(Path::new("")).join(BUILDS);
        let all: Vec<u8> = artifacts.iter().flat_map(|a| a.bytes.iter().copied()).collect();
        let dir = builds.join(Hash::of(&all).digest());
        let mut objects = Vec::new();
        for artifact in &artifacts {
            objects.push(self.object(&dir, artifact, &target)?);
        }
        let stem = root.file_stem().map(PathBuf::from).unwrap_or_default();
        let output = output.map_or_else(|| builds.join(stem), Path::to_path_buf);
        let plan = LinkPlan {
            inputs: objects,
            output: output.clone(),
            kind: OutputKind::Executable,
            entry: target.entry_symbol,
            regions: Vec::new(),
            sections: Vec::new(),
        };
        let registry = Registry::with_defaults();
        let Some(linker) = registry.linker(target.default_linker) else {
            let msg = Msg::new()
                .text("no linker ")
                .code(target.default_linker.as_str())
                .text(" is in this toolchain");
            return Err(self.diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        linker.link(&plan, &target, &mut self.diag)?;
        Ok(output)
    }

    /// The object file of `artifact`, written in `dir`: itself, or what the target's C compiler
    /// makes of it.
    fn object(
        &mut self,
        dir: &Path,
        artifact: &Artifact,
        target: &vun_target::TargetSpec,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        let path = dir.join(&artifact.name);
        path.put(&artifact.bytes).map_err(|e| {
            let msg = Msg::new().text("cannot write ").code(path.display().to_string());
            self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e))
        })?;
        match artifact.kind {
            Emits::Object => Ok(path),
            Emits::Source => {
                let compiler = &target.c.compiler;
                let object = path.with_extension(OBJECT);
                let mut args: Vec<String> = compiler.flags.iter().map(|f| f.as_str().to_owned()).collect();
                args.extend([
                    "-c".to_owned(),
                    path.display().to_string(),
                    "-o".to_owned(),
                    object.display().to_string(),
                ]);
                let inv = Invocation {
                    args,
                    inputs: vec![path.clone()],
                    outputs: vec![object.clone()],
                    ..Invocation::default()
                };
                let command = compiler.command.as_str();
                Executable::new(command, command).check(&inv).map_err(|failure| {
                    let msg =
                        Msg::new().code(command).text(" could not compile ").code(path.display().to_string());
                    self.diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
                })?;
                Ok(object)
            }
            Emits::Assembly | Emits::Jit => Err(self.diag.error(Diagnostic::error(
                errors::BACKEND_UNAVAILABLE,
                Msg::new().text("building from assembly or code in memory is not supported yet"),
            ))),
        }
    }
}
