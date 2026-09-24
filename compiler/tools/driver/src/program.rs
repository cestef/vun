//! Building a program from a root file (spec 13, section 1): the chosen backend's artifacts,
//! written under `.vun/build/` next to the root file, by their content; each source compiled by
//! the target's C compiler, in parallel, with the frame sizes it reports; then the target's
//! linker.

use std::path::{Path, PathBuf};

use vun_backend::{Artifact, Emits};
use vun_command::{Param, ParamKind};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Edit;
use vun_hash::Hash;
use vun_link::{LinkPlan, OutputKind};
use vun_lir::FuncId;
use vun_span::{FileId, SourceMap};
use vun_target::{CCompiler, FrameSizes};
use vun_tool::{Arg, Executable, Failure, Invocation, Tool, gcc};

use crate::Session;
use crate::codegen::{Coded, Uncoded};

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
/// The extension of the frame sizes `-fstack-usage` writes beside an object.
const STACK_USAGE: &str = "su";

/// A program compiled to objects.
pub struct Compiled {
    pub coded: Coded,
    pub objects: Vec<PathBuf>,
    /// Each function's frame size in bytes, when the tools report them.
    pub frames: Vec<(FuncId, u64)>,
    /// Where the build keeps its files.
    pub builds: PathBuf,
}

impl Session {
    /// Compiles the program rooted at `file` to objects.
    ///
    /// # Errors
    /// Returns once what stopped the build is reported.
    ///
    /// # Panics
    /// Never: a compile step does not panic, and the backend that coded it is registered.
    pub fn compile(&mut self, file: FileId, backend: Option<&str>) -> Result<Compiled, ErrorGuaranteed> {
        let coded = self.code(file, backend).map_err(|uncoded| match uncoded {
            Uncoded::Reported(guar) => guar,
            Uncoded::Unlowered(unlowered) => {
                let msg = Msg::new().text("the program does not lower to LIR: ").text(unlowered.to_string());
                self.diag.error(Diagnostic::error(errors::BACKEND_UNLOWERED, msg).at(unlowered.at, None))
            }
        })?;
        let root = PathBuf::from(&self.sources.file(file).path);
        let builds = root.parent().unwrap_or(Path::new("")).join(BUILDS);
        let all: Vec<u8> = coded.artifacts.iter().flat_map(|a| a.bytes.iter().copied()).collect();
        let dir = builds.join(Hash::of(&all).digest());
        for artifact in &coded.artifacts {
            let path = dir.join(&artifact.name);
            path.put(&artifact.bytes).map_err(|e| {
                let msg = Msg::new().text("cannot write ").code(path.display().to_string());
                self.diag.error(Diagnostic::error(errors::IO_UNWRITABLE_FILE, msg).caused_by(&e))
            })?;
        }
        let compiler = &coded.target.c.compiler;
        let sources: Vec<&Artifact> = coded.artifacts.iter().filter(|a| a.kind == Emits::Source).collect();
        let results: Vec<Result<PathBuf, Failure>> = std::thread::scope(|scope| {
            let runs: Vec<_> = sources
                .iter()
                .map(|a| scope.spawn(|| Self::compile_source(compiler, &dir.join(&a.name))))
                .collect();
            runs.into_iter().map(|run| run.join().expect("a compile step does not panic")).collect()
        });
        let mut objects = Vec::new();
        for (artifact, result) in sources.iter().zip(results) {
            objects.push(result.map_err(|failure| {
                let msg = Msg::new()
                    .code(compiler.command.as_str())
                    .text(" could not compile ")
                    .code(artifact.name.display().to_string());
                self.diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
            })?);
        }
        objects.extend(coded.artifacts.iter().filter(|a| a.kind == Emits::Object).map(|a| dir.join(&a.name)));
        if coded.artifacts.iter().any(|a| matches!(a.kind, Emits::Assembly | Emits::Jit)) {
            let msg = Msg::new().text("building from assembly or code in memory is not supported yet");
            return Err(self.diag.error(Diagnostic::error(errors::BACKEND_UNAVAILABLE, msg)));
        }
        let frames = match compiler.frame_sizes {
            FrameSizes::StackUsage => {
                let report: Vec<u8> = objects
                    .iter()
                    .filter_map(|o| std::fs::read(o.with_extension(STACK_USAGE)).ok())
                    .flatten()
                    .collect();
                let backend = self.registry.backends().nth(coded.backend).expect("the backend that coded it");
                backend.frame_sizes(coded.unit(), &report)
            }
            FrameSizes::None => Vec::new(),
        };
        Ok(Compiled { coded, objects, frames, builds })
    }

    /// Prints the frame size of each function of the program rooted at `file`, as its tools
    /// report them.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn dump_frames(
        &mut self,
        file: FileId,
        backend: Option<&str>,
        out: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        let Ok(compiled) = self.compile(file, backend) else { return Ok(()) };
        for (id, bytes) in compiled.frames {
            let origin = compiled.coded.unit().functions[id].origin;
            let at = self.sources.locate(self.origins.source_span(origin));
            writeln!(out, "{id} at {at}: {bytes} bytes")?;
        }
        Ok(())
    }

    /// The object `compiler` makes of the source at `path`, beside it.
    fn compile_source(compiler: &CCompiler, path: &Path) -> Result<PathBuf, Failure> {
        let object = path.with_extension(OBJECT);
        let mut args: Vec<Arg> = compiler.flags.iter().map(|&f| Arg::Text(f)).collect();
        if compiler.frame_sizes == FrameSizes::StackUsage {
            args.push(gcc::STACK_USAGE.into());
        }
        args.extend([
            gcc::COMPILE.into(),
            Arg::Path(path.to_path_buf()),
            gcc::OUTPUT.into(),
            Arg::Path(object.clone()),
        ]);
        let inv = Invocation {
            args,
            inputs: vec![path.to_path_buf()],
            outputs: vec![object.clone()],
            ..Invocation::default()
        };
        let command = compiler.command.as_str();
        Executable::new(command, command).check(&inv)?;
        Ok(object)
    }

    /// Builds the program rooted at `file` into `output`, or into the builds, named as the file.
    ///
    /// # Errors
    /// Returns once what stopped the build is reported.
    pub fn build(
        &mut self,
        file: FileId,
        backend: Option<&str>,
        output: Option<&Path>,
    ) -> Result<PathBuf, ErrorGuaranteed> {
        let compiled = self.compile(file, backend)?;
        let target = &compiled.coded.target;
        let root = PathBuf::from(&self.sources.file(file).path);
        let stem = root.file_stem().map(PathBuf::from).unwrap_or_default();
        let output = output.map_or_else(|| compiled.builds.join(stem), Path::to_path_buf);
        let plan = LinkPlan {
            inputs: compiled.objects.clone(),
            output: output.clone(),
            kind: OutputKind::Executable,
            entry: target.entry_symbol,
            regions: Vec::new(),
            sections: Vec::new(),
        };
        let Some(linker) = self.registry.linker(target.default_linker) else {
            let msg = Msg::new()
                .text("no linker ")
                .code(target.default_linker.as_str())
                .text(" is in this toolchain");
            return Err(self.diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        linker.link(&plan, target, &mut self.diag)?;
        Ok(output)
    }
}
