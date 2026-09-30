//! Why no backend builds, as a diagnostic.

use vun_diag::{ChildKind, Diagnostic, Msg, errors};
use vun_intern::Symbol;
use vun_suggest::Lookup;
use vun_target::{TargetSpec, Unsupported};

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
            &Unchosen::Named(name, Unfit::Unsupported(why)) => {
                let msg = Msg::new().text("backend ").code(name.as_str()).append(Self::unsupported(why));
                self.fitting(target, Diagnostic::error(errors::BACKEND_UNSUPPORTED, msg))
            }
            Unchosen::Preferred(profile, tried) => {
                let msg = Msg::new()
                    .text("no backend the target prefers for ")
                    .text(profile.name())
                    .text(" builds can build");
                let d = tried
                    .iter()
                    .fold(Diagnostic::error(errors::BACKEND_UNCHOSEN, msg), |d, &(name, unfit)| {
                        d.child(ChildKind::Note, Self::unfit(name, unfit))
                    });
                self.fitting(target, d)
            }
        }
    }

    fn unfit(name: Symbol, unfit: Unfit) -> Msg {
        let why = match unfit {
            Unfit::Absent => Msg::new().text(" is not in this toolchain"),
            Unfit::Planned => Msg::new().text(" cannot generate code yet"),
            Unfit::Unsupported(why) => Self::unsupported(why),
        };
        Msg::new().code(name.as_str()).append(why)
    }

    /// Why a backend does not generate code for a target.
    fn unsupported(why: Unsupported) -> Msg {
        let msg = Msg::new().text(" does not generate ");
        match why {
            Unsupported::Arch(arch) => msg.text("code for ").code(arch.as_str()),
            Unsupported::ObjectFormat(format) => msg.text("objects of format ").code(format.as_str()),
            Unsupported::MemoryRegions => msg.text("code placed in memory regions"),
            Unsupported::Output(output) => msg.text("programs as ").code(output.name()),
        }
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
