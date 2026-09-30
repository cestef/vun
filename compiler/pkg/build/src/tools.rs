//! The machine's tools file (spec 12, section 6): what this machine runs to build and run
//! programs for a target, where the target's own tools do not fit it, such as a cross C compiler
//! and an emulator. It is read as literal data, without evaluating anything:
//!
//! ```vun
//! tools := .{
//!     .targets = .{
//!         .{
//!             .target = linux.arm64.kernel,
//!             .compiler = "aarch64-linux-gnu-gcc",
//!             .archiver = "aarch64-linux-gnu-ar",
//!             .sysroot = "/usr/aarch64-linux-gnu",
//!             .runner = .{ "qemu-aarch64", "-L", "/usr/aarch64-linux-gnu" },
//!         },
//!     },
//! };
//! ```

use std::path::PathBuf;

use vun_diag::{DiagCx, Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_span::Span;
use vun_syntax::literal::{Decl, Literal};
use vun_target::{BuildsFor, TargetSpec};

use crate::manifest::record::{Fields, Record};

/// What the machine runs for each target it names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tools {
    pub targets: Vec<TargetTools>,
}

/// What the machine runs for one target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetTools {
    /// The target, by its path in the package `targets`, as `--target` names it.
    pub target: Vec<Symbol>,
    /// The C compiler run instead of the target's.
    pub compiler: Option<Symbol>,
    /// The archiver run instead of the target's.
    pub archiver: Option<Symbol>,
    /// Flags every run of the C compiler takes, compiling and linking.
    pub flags: Vec<Symbol>,
    /// The directory of the target system's headers and libraries: a sysroot, an SDK.
    pub sysroot: Option<Symbol>,
    /// The command a program is run through, its path and arguments after: an emulator. Empty
    /// where programs run as they are.
    pub runner: Vec<Symbol>,
    pub span: Span,
}

mod field {
    pub(super) const TARGETS: &str = "targets";
    pub(super) const TARGET: &str = "target";
    pub(super) const COMPILER: &str = "compiler";
    pub(super) const ARCHIVER: &str = "archiver";
    pub(super) const FLAGS: &str = "flags";
    pub(super) const SYSROOT: &str = "sysroot";
    pub(super) const RUNNER: &str = "runner";

    pub(super) const TOOLCHAIN: &[&str] = &[TARGETS];
    pub(super) const TOOLS: &[&str] = &[TARGET, COMPILER, ARCHIVER, FLAGS, SYSROOT, RUNNER];
}

impl Tools {
    /// The environment variable naming the file, instead of the user's.
    pub const ENV: &str = "VUN_TOOLS";
    /// The directory of `vun` in the user's configuration, and the file there.
    pub const CONFIG: &str = "vun";
    pub const FILE: &str = "tools.vun";
    /// The declaration holding it.
    pub const DECL: &str = "tools";

    /// The machine's file: the one [`Self::ENV`] names, or `vun/tools.vun` in the user's
    /// configuration directory (XDG on Unix, including macOS; the known folder on Windows).
    #[must_use]
    pub fn path() -> Option<PathBuf> {
        use etcetera::BaseStrategy;
        let named = std::env::var_os(Self::ENV).map(PathBuf::from);
        named.or_else(|| {
            let base = etcetera::choose_base_strategy().ok()?;
            Some(base.config_dir().join(Self::CONFIG).join(Self::FILE))
        })
    }

    /// Reads the tools from the declarations of their file; one without the declaration holds
    /// nothing.
    pub fn read(decls: &[Decl<'_>], cx: &mut DiagCx<'_>) -> Option<Self> {
        let Some(decl) = decls.iter().find(|d| d.name == Self::DECL) else { return Some(Self::default()) };
        let fields = Fields::new(decl, field::TOOLCHAIN, cx)?;
        let targets = fields.get(field::TARGETS).map_or(Some(Vec::new()), |d| d.items(TargetTools::read, cx));
        fields.finish(cx);
        Some(Self { targets: targets? })
    }
}

impl TargetTools {
    fn read(decl: &Decl<'_>, cx: &mut DiagCx<'_>) -> Option<Self> {
        let mut fields = Fields::new(decl, field::TOOLS, cx)?;
        let target = fields.require(field::TARGET).and_then(|d| match &d.value {
            Literal::Path(path) => Some(path.iter().map(|&n| Symbol::intern(n)).collect()),
            Literal::Name(name) => Some(vec![Symbol::intern(name)]),
            _ => d.invalid(
                Msg::new()
                    .text("a path in the package ")
                    .code("targets")
                    .text(", as ")
                    .code("linux.arm64.kernel"),
                cx,
            ),
        });
        let compiler = fields
            .get(field::COMPILER)
            .map_or(Some(None), |d| d.string(cx).map(|s| Some(Symbol::intern(&s))));
        let archiver = fields
            .get(field::ARCHIVER)
            .map_or(Some(None), |d| d.string(cx).map(|s| Some(Symbol::intern(&s))));
        let flags = fields.get(field::FLAGS).map_or(Some(Vec::new()), |d| d.symbols(cx));
        let sysroot =
            fields.get(field::SYSROOT).map_or(Some(None), |d| d.string(cx).map(|s| Some(Symbol::intern(&s))));
        let runner = fields.get(field::RUNNER).map_or(Some(Vec::new()), |d| d.symbols(cx));
        fields.finish(cx);
        Some(Self {
            target: target?,
            compiler: compiler?,
            archiver: archiver?,
            flags: flags?,
            sysroot: sysroot?,
            runner: runner?,
            span: decl.span,
        })
    }

    /// Makes `spec` built with these tools: their C compiler, their flags on each of its runs, and
    /// their system root.
    pub fn apply(&self, spec: &mut TargetSpec) {
        let compiler = &mut spec.c.compiler;
        if let Some(command) = self.compiler {
            compiler.command = command;
            compiler.builds_for = BuildsFor::Target;
        }
        if let Some(archiver) = self.archiver {
            compiler.archiver = archiver;
        }
        if let Some(sysroot) = self.sysroot {
            compiler.sysroot = Some(sysroot);
        }
        compiler.flags.extend(&self.flags);
        compiler.link_flags.extend(&self.flags);
    }

    /// Reports that the target these tools name has none.
    pub fn unnamed(&self, why: &(dyn std::error::Error + 'static), cx: &mut DiagCx<'_>) {
        let path =
            self.target.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(vun_grammar::PATH_SEPARATOR);
        let msg = Msg::new().text("the tools file names no target ").code(path);
        cx.error_at(self.span, Diagnostic::error(errors::TARGET_UNCHOSEN, msg).caused_by(why));
    }
}
