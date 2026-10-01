//! Options given values by a project: after the command line and the environment, before the
//! command's own defaults.

use vun_diag::Diagnostic;

use super::Selected;
use crate::{Args, HELP, ParamKind, Settings, VERSION};

impl Args {
    /// Gives each option of the command selected that neither the command line nor the
    /// environment gave a value the values `settings` have for it. A flag a setting holds is
    /// given when the setting is `true`. Settings naming no option of the command are left to
    /// the caller to report ([`Selected::accepts`]).
    ///
    /// # Errors
    /// Returns a usage diagnostic for a value outside what the option takes.
    pub fn defaulted<Cx>(
        &mut self,
        selected: &Selected<'_, Cx>,
        settings: &Settings,
    ) -> Result<(), Box<Diagnostic>> {
        for setting in settings.of(selected.last().name) {
            let Some(param) = selected.options().find(|p| p.name == setting.option) else { continue };
            if param.is(&HELP) || param.is(&VERSION) || self.value(param).is_some() || self.flag(param) {
                continue;
            }
            match param.kind {
                ParamKind::Flag => {
                    if setting.values.first().is_some_and(|v| v == "true") {
                        self.flags.push(param.name);
                    }
                }
                _ => {
                    for value in &setting.values {
                        self.give(param, value)?;
                    }
                }
            }
        }
        Ok(())
    }
}
