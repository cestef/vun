//! The machine's toolchain (spec 12, section 6): once the target is chosen, the tools the
//! machine's toolchain file names for it replace the target's own (a cross C compiler, its
//! flags), and programs built for it run through its runner (an emulator).

use std::path::{Component, Path};
use std::process::Command;

use vun_build::toolchain::Toolchain;
use vun_diag::DiagCx;
use vun_hir::db::FileRef;
use vun_sema::target::{Choice, Chosen};
use vun_target::TargetSpec;
use vun_trace::tracing;

use crate::Session;

impl Session {
    /// `spec`, chosen for `key`, with the tools the machine's toolchain file names for it.
    pub(crate) fn equip(&mut self, key: &FileRef, mut spec: TargetSpec) -> TargetSpec {
        let Some(toolchain) = self.machine_toolchain() else { return spec };
        for tools in toolchain.targets {
            match self.db.get::<Chosen>(&(key.clone(), Choice::Path(tools.target.clone()))) {
                Ok(named) if named.spec == spec => {
                    tracing::info!(compiler = ?tools.compiler, runner = ?tools.runner, "tools of the machine");
                    tools.apply(&mut spec);
                    self.runner = tools.runner;
                    return spec;
                }
                Ok(_) => {}
                Err(why) => tools.unnamed(&why, &mut DiagCx { diag: &mut self.diag, origins: self.origins }),
            }
        }
        spec
    }

    /// The machine's toolchain file, read; `None` where there is none or it does not read.
    fn machine_toolchain(&mut self) -> Option<Toolchain> {
        let path = self.toolchain.clone().filter(|p| p.exists())?;
        let text = self.read(&path)?;
        let file = self.sources.add(path.display().to_string(), text);
        self.declarations(file, Toolchain::read)
    }

    /// The command running `program`: through the target's runner, if it has one. A relative
    /// path is taken from the working directory, never looked up in `PATH`.
    #[must_use]
    pub fn command(&self, program: &Path) -> Command {
        let here = Path::new(Component::CurDir.as_os_str());
        let program = if program.is_relative() { here.join(program) } else { program.to_path_buf() };
        match self.runner.split_first() {
            Some((runner, args)) => {
                let mut command = Command::new(runner.as_str());
                command.args(args.iter().map(|a| a.as_str())).arg(program);
                command
            }
            None => Command::new(program),
        }
    }
}
