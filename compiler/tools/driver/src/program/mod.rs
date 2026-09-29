//! Building a program from a root file (spec 13, section 1): the chosen backend's artifacts,
//! written under `.vun/build/` next to the root file, by their content; each source compiled by
//! the target's C compiler, in parallel, with the frame sizes it reports; then the target's
//! linker. What is built is kept by its content (`cache`): a build finding it skips the step.

mod cache;

use std::path::{Path, PathBuf};

use vun_backend::{Artifact, Emits};
use vun_command::{Param, PathKind, Values};
use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::{Edit, Part};
use vun_link::LinkPlan;
use vun_lir::FuncId;
use vun_origin::Resolve;
use vun_par::Parallel;
use vun_span::{FileId, SourceMap};
use vun_target::{CCompiler, FrameSizes, Profile, Support, Unsupported};
use vun_tool::{Arg, Executable, Failure, Invocation, Tool, gcc};
use vun_trace::{Activity, tracing};

use crate::Session;
use crate::codegen::{Coded, Listed, Uncoded};

pub const OUTPUT: Param = Param::option("output")
    .short('o')
    .about("where the program is written")
    .values(Values::Path(PathKind::File));

/// Where builds keep their files, under the root file's directory (spec 12, section 1).
const BUILDS: &str = "build";
/// The extension of the frame sizes `-fstack-usage` writes beside an object.
const STACK_USAGE: &str = "su";

/// The steps of a build past the checks, as timed.
const CODE: Activity = Activity::stage("code");
const COMPILE: Activity = Activity::stage("compile");
const LINK: Activity = Activity::stage("link");

/// A program compiled to objects.
pub struct Compiled {
    pub coded: Coded,
    pub objects: Vec<PathBuf>,
    /// Each function's frame size in bytes, when the backend or its tools report them.
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
    pub fn compile(&mut self, file: FileId) -> Result<Compiled, ErrorGuaranteed> {
        // Only a program its checks accept is built.
        self.check(file);
        if let Some(failed) = self.diag.failed() {
            return Err(failed);
        }
        let timer = CODE.start();
        let coded = self.code(file, Listed::No).map_err(|uncoded| match uncoded {
            Uncoded::Reported(guar) => guar,
            Uncoded::Unlowered(unlowered) => self.diag.error(unlowered.diagnostic()),
        })?;
        drop(timer);
        tracing::info!(backend = coded.backend.0, artifacts = coded.artifacts.len(), "coded");
        let _timer = COMPILE.start();
        let root = PathBuf::from(&self.sources.file(file).path);
        let builds = root.parent().unwrap_or(Path::new("")).join(vun_fs::WORK).join(BUILDS);
        let compiler = &coded.target.c.compiler;
        let profile = self.codegen.profile;
        let dir = builds.join(cache::Objects::key(&coded.artifacts, compiler, profile).digest());
        // A directory of this key holds these artifacts already.
        for artifact in coded.artifacts.iter().filter(|a| !dir.join(a.file()).exists()) {
            let path = dir.join(artifact.file());
            path.put(&artifact.bytes).map_err(|e| self.diag.error(Diagnostic::unwritable(&path, &e)))?;
        }
        let sources: Vec<&Artifact> = coded.artifacts.iter().filter(|a| a.kind == Emits::Source).collect();
        let results = sources.par_map(|a| {
            tracing::debug!(source = %a.file().display(), compiler = compiler.command.as_str(), "compiling");
            Self::compile_source(compiler, profile, self.codegen.output, &dir, a.part)
        });
        let mut objects = Vec::new();
        for (artifact, result) in sources.iter().zip(results) {
            objects.push(result.map_err(|failure| {
                let msg = Msg::new()
                    .code(compiler.command.as_str())
                    .text(" could not compile ")
                    .code(artifact.file().display().to_string());
                self.diag.error(Diagnostic::error(errors::BACKEND_TOOL, msg).caused_by(&failure))
            })?);
        }
        objects
            .extend(coded.artifacts.iter().filter(|a| a.kind == Emits::Object).map(|a| dir.join(a.file())));
        if coded.artifacts.iter().any(|a| matches!(a.kind, Emits::Assembly | Emits::Jit)) {
            let msg = Msg::new().text("building from assembly or code in memory is not supported yet");
            return Err(self.diag.error(Diagnostic::error(errors::BACKEND_UNAVAILABLE, msg)));
        }
        // Frame sizes from the backend, else from what the tools building its artifacts report.
        let frames = match compiler.frame_sizes {
            _ if !coded.frames.is_empty() => coded.frames.clone(),
            FrameSizes::StackUsage => {
                let report: Vec<u8> = objects
                    .iter()
                    .filter_map(|o| std::fs::read(o.with_extension(STACK_USAGE)).ok())
                    .flatten()
                    .collect();
                let backend =
                    self.registry.backend(coded.backend.symbol()).expect("the backend that coded it");
                backend.frame_sizes(&coded.unit, &report)
            }
            FrameSizes::None => Vec::new(),
        };
        self.check_stacks(&coded.unit, &frames, coded.backend.0)?;
        Ok(Compiled { coded, objects, frames, builds })
    }

    /// Prints the frame size of each function of the program rooted at `file`, as its tools
    /// report them.
    ///
    /// # Errors
    /// Returns an error if `out` cannot be written.
    pub fn dump_frames(&mut self, file: FileId, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        let Ok(compiled) = self.compile(file) else { return Ok(()) };
        for (id, bytes) in compiled.frames {
            let origin = compiled.coded.unit.functions[id].origin;
            let at = self.sources.locate(self.source_span(origin));
            writeln!(out, "{id} at {at}: {bytes} bytes")?;
        }
        Ok(())
    }

    /// The object `compiler` makes of the source of `part` in `dir`, beside it, with the flags
    /// of `profile`: made under another name and renamed, so an object there was made whole, and
    /// kept by the next build of the same key.
    fn compile_source(
        compiler: &CCompiler,
        profile: Profile,
        output: vun_link::Output,
        dir: &Path,
        part: vun_backend::Part,
    ) -> Result<PathBuf, Failure> {
        let path = dir.join(part.file(Emits::Source));
        let object = dir.join(part.file(Emits::Object));
        if object.exists() {
            return Ok(object);
        }
        // Made under this process's own name, so builds of one key at once never meet.
        let made = object.part();
        let optimize = match profile {
            Profile::Debug => &[][..],
            Profile::Release => &compiler.optimize,
        };
        let mut args: Vec<Arg> = compiler.flags.iter().chain(optimize).map(|&f| Arg::Text(f)).collect();
        if compiler.frame_sizes == FrameSizes::StackUsage {
            args.push(gcc::STACK_USAGE.into());
        }
        if output == vun_link::Output::Shared {
            args.push(gcc::PIC.into());
        }
        args.extend([
            gcc::COMPILE.into(),
            Arg::Path(path.clone()),
            gcc::OUTPUT.into(),
            Arg::Path(made.clone()),
        ]);
        let inv =
            Invocation { args, inputs: vec![path], outputs: vec![made.clone()], ..Invocation::default() };
        let command = compiler.command.as_str();
        Executable::new(command, command).check(&inv)?;
        // A compiler writes no report for a source without a function it lays a frame out for.
        let report = made.with_extension(STACK_USAGE);
        if compiler.frame_sizes == FrameSizes::StackUsage && report.exists() {
            std::fs::rename(report, object.with_extension(STACK_USAGE)).map_err(Failure::Start)?;
        }
        std::fs::rename(&made, &object).map_err(Failure::Start)?;
        Ok(object)
    }

    /// Builds the program rooted at `file` into `output`, or into the builds, named as the file.
    ///
    /// # Errors
    /// Returns once what stopped the build is reported.
    pub fn build(&mut self, file: FileId, output: Option<&Path>) -> Result<PathBuf, ErrorGuaranteed> {
        let compiled = self.compile(file)?;
        let target = &compiled.coded.target;
        let root = PathBuf::from(&self.sources.file(file).path);
        let kind = self.codegen.output;
        let stem = root.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let output = output
            .map_or_else(|| kind.file(&compiled.builds, &stem, target.object_format), Path::to_path_buf);
        let plan = LinkPlan {
            inputs: compiled.objects.clone(),
            output: output.clone(),
            kind,
            // A library's exports are its interface: nothing starts it.
            entry: target.entry_symbol.filter(|_| !kind.library()),
            regions: self.memory.clone(),
            sections: Vec::new(),
        };
        let named = self.codegen.linker.unwrap_or_else(|| self.registry.linker_for(kind, target));
        let Some(linker) = self.registry.linker(named) else {
            let msg = Msg::new().text("no linker ").code(named.as_str()).text(" is in this toolchain");
            return Err(self.diag.error(Diagnostic::error(errors::LINK_UNAVAILABLE, msg)));
        };
        if let Support::No(unsupported) = linker.info().supports(target, &plan) {
            let msg = Msg::new().text("linker ").code(named.as_str()).text(" does not link ");
            let msg = match unsupported {
                Unsupported::ObjectFormat(format) => msg.text("objects of format ").code(format.as_str()),
                Unsupported::MemoryRegions => msg.text("the placement of sections in memory regions"),
                Unsupported::Arch(arch) => msg.text("code for ").code(arch.as_str()),
                Unsupported::Output(output) => msg.text("programs as ").code(output.name()),
            };
            return Err(self.diag.error(Diagnostic::error(errors::LINK_UNSUPPORTED, msg)));
        }
        let _timer = LINK.start();
        let stamp = cache::Stamp::of(&compiled.builds, linker.info().id, target, &plan);
        if stamp.holds() {
            tracing::info!(output = %output.display(), "linked already");
            return Ok(output);
        }
        linker.link(&plan, target, &mut self.diag)?;
        stamp.record();
        tracing::info!(output = %output.display(), objects = plan.inputs.len(), "linked");
        Ok(output)
    }
}
