//! Giving a file to a device by the ways its plan lists (`std.build.Delivery`), each tried until
//! one reaches a device.

use std::fmt;
use std::io;
use std::path::Path;

use vun_diag::{Diagnostic, ErrorGuaranteed, Msg, errors};
use vun_fs::Mounts;
use vun_sema_plan::{Argument, Delivery, Invocation};
use vun_tool::{Arg, Executable, Failure, Invocation as Run, Tool};

use crate::Session;

/// Why a way did not reach a device.
#[derive(Debug)]
enum Missed {
    NoProgram,
    Program(Failure),
    NoDrive,
    Copy(io::Error),
}

impl fmt::Display for Missed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NoProgram => "it is not on the search path",
            Self::Program(_) => "it did not write the file",
            Self::NoDrive => "no mounted drive holds it",
            Self::Copy(_) => "the file could not be copied to the drive",
        })
    }
}

impl std::error::Error for Missed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Program(e) => Some(e),
            Self::Copy(e) => Some(e),
            Self::NoProgram | Self::NoDrive => None,
        }
    }
}

/// Giving a file to a device.
trait Deliver {
    fn deliver(&self, file: &Path) -> Result<(), Missed>;
}

impl Deliver for Delivery {
    fn deliver(&self, file: &Path) -> Result<(), Missed> {
        match self {
            Self::Drive { mark } => {
                let drive = Mounts::all().into_iter().find(|mount| mount.join(mark.as_str()).is_file());
                let drive = drive.ok_or(Missed::NoDrive)?;
                let name = file.file_name().unwrap_or(file.as_os_str());
                std::fs::copy(file, drive.join(name)).map(drop).map_err(Missed::Copy)
            }
            Self::Run(Invocation { command, args }) => {
                let program = Executable::search(command.as_str()).ok_or(Missed::NoProgram)?;
                let args = args.iter().map(|arg| match arg {
                    Argument::Text(text) => Arg::Text(*text),
                    Argument::File => Arg::Path(file.to_path_buf()),
                });
                let run = Run { args: args.collect(), ..Run::default() };
                Executable::new(command.as_str(), program).check(&run).map(drop).map_err(Missed::Program)
            }
        }
    }
}

impl Session {
    /// Gives `file` to a device by the first of `ways` that reaches one, and returns it; when none
    /// does, each says why.
    ///
    /// # Errors
    /// Returns once that is reported.
    pub fn deliver<'w>(
        &mut self,
        file: &Path,
        ways: &'w [Delivery],
    ) -> Result<&'w Delivery, ErrorGuaranteed> {
        let mut missed = Vec::new();
        for way in ways {
            match way.deliver(file) {
                Ok(()) => return Ok(way),
                Err(why) => missed.push((way, why)),
            }
        }
        let msg = Msg::new()
            .text("cannot write ")
            .code(crate::subject::Subject::relative(file))
            .text(" to a device");
        let help = Msg::new().text("put the device in its boot loader mode, and run the command again");
        let d = missed.iter().fold(Diagnostic::error(errors::PLAN_UNDELIVERED, msg), |d, (way, why)| {
            let d = d.note(Msg::new().text(way.to_string()).text(": ").text(why.to_string()));
            match std::error::Error::source(why) {
                Some(source) => d.caused_by(source),
                None => d,
            }
        });
        Err(self.diag.error(d.help(help)))
    }
}
