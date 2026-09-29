//! The Zed extension of Vun: starts the language server `server.rs` names, found on the
//! worktree's path.

use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

mod server;

struct Vun;

impl zed::Extension for Vun {
    fn new() -> Self {
        Self
    }

    fn language_server_command(&mut self, _: &LanguageServerId, worktree: &Worktree) -> Result<zed::Command> {
        let command = worktree.which(server::COMMAND).ok_or_else(|| format!("`{}` is not on the path", server::COMMAND))?;
        let args = server::ARGS.iter().map(|&a| a.to_owned()).collect();
        Ok(zed::Command { command, args, env: worktree.shell_env() })
    }
}

zed::register_extension!(Vun);
