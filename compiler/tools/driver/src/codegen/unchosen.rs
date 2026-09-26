//! Why no backend builds, as a diagnostic.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_suggest::Lookup;
use vun_target::{Profile, TargetSpec};

use crate::Session;
use crate::registry::{Unchosen, Unfit};

impl Session {
    pub(super) fn unchosen(&self, unchosen: &Unchosen, target: &TargetSpec) -> Diagnostic {
        match unchosen {
            Unchosen::Named(name, Unfit::Absent) => {
                let names: Vec<&str> = self.registry.backends().map(|b| b.info().id.0).collect();
                let near = names.lookup(name.as_str(), |n| n).err().flatten();
                let msg = Msg::new().text("unknown backend ").code(name.as_str());
                let help = near.map_or_else(
                    || Msg::new().text("backends: ").code_list(names.iter().copied()),
                    Msg::did_you_mean,
                );
                Diagnostic::error(errors::CLI_INVALID_VALUE, msg).child(ChildKind::Help, help)
            }
            Unchosen::Named(name, Unfit::Planned) => {
                let msg = Msg::new().text("backend ").code(name.as_str()).text(" cannot generate code yet");
                self.fitting(target, Diagnostic::error(errors::BACKEND_UNAVAILABLE, msg))
            }
            Unchosen::Named(name, Unfit::Arch) => {
                let msg = Msg::new()
                    .text("backend ")
                    .code(name.as_str())
                    .text(" does not generate code for ")
                    .code(target.arch.as_str());
                self.fitting(target, Diagnostic::error(errors::BACKEND_UNSUPPORTED, msg))
            }
            Unchosen::Preferred(profile, tried) => {
                let which = match profile {
                    Profile::Debug => "debug",
                    Profile::Release => "release",
                };
                let msg = Msg::new()
                    .text("no backend the target prefers for ")
                    .text(which)
                    .text(" builds can build");
                let d = tried
                    .iter()
                    .fold(Diagnostic::error(errors::BACKEND_UNCHOSEN, msg), |d, &(name, unfit)| {
                        d.child(ChildKind::Note, Self::unfit(name, unfit, target.arch))
                    });
                self.fitting(target, d)
            }
        }
    }

    fn unfit(name: Symbol, unfit: Unfit, arch: Symbol) -> Msg {
        let why = match unfit {
            Unfit::Absent => Msg::new().text(" is not in this toolchain"),
            Unfit::Planned => Msg::new().text(" cannot generate code yet"),
            Unfit::Arch => Msg::new().text(" does not generate code for ").code(arch.as_str()),
        };
        Msg::new().code(name.as_str()).append(why)
    }

    /// `d`, with the backends that could build for `target` named.
    fn fitting(&self, target: &TargetSpec, d: Diagnostic) -> Diagnostic {
        let fitting: Vec<&str> = self.registry.fitting(target).map(|b| b.info().id.0).collect();
        if fitting.is_empty() {
            return d;
        }
        d.child(ChildKind::Help, Msg::new().text("backends that can: ").code_list(fitting.iter().copied()))
    }
}
