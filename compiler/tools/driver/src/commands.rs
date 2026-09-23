//! The `overt` command tree (spec 12, section 7), shared by every front end of the driver.

use std::io::{self, Write};
use std::path::Path;

use overt_command::{Action, Args, Command, Outcome, Param, ParamKind, Program};

use crate::{Registry, Session, VERSION};

pub const PROGRAM: Program<Session> = Program {
    version: VERSION,
    root: Command {
        name: "overt",
        about: "The Overt toolchain.",
        params: &[],
        action: Action::Group(&[
            planned("new", "create a package"),
            planned("build", "build the package"),
            planned("run", "build and run a program"),
            planned("check", "analyze without generating code"),
            planned("test", "run tests, properties and simulations"),
            planned("bench", "run benchmarks"),
            planned("fuzz", "fuzz a test with coverage guidance"),
            planned("fmt", "format source files"),
            planned("fix", "apply fixes from diagnostics"),
            planned("doc", "generate documentation"),
            planned("explain", "explain a diagnostic"),
            group("dump", "print an intermediate representation with origins", DUMPS),
            group(
                "pkg",
                "dependencies, releases and API diffs",
                &[
                    planned("add", "add a dependency"),
                    planned("update", "raise dependency versions"),
                    planned("fetch", "download dependencies into the local store"),
                    planned("vendor", "copy dependencies into vendor/"),
                    planned("why", "explain why a package is in the graph"),
                    planned("tag", "release workspace members"),
                    planned("api", "print the public API, or diff it against a release"),
                ],
            ),
            group(
                "tool",
                "C bindings, C headers, register packages",
                &[
                    planned("bind", "generate declarations from a C header"),
                    planned("header", "emit a C header for a library"),
                    planned("svd", "generate a register package from an SVD file"),
                ],
            ),
            group(
                "target",
                "flash, attach to and debug devices",
                &[
                    planned("flash", "write a program to a device"),
                    planned("attach", "stream logs from a device"),
                    planned("debug", "run a GDB server for a device"),
                ],
            ),
            group(
                "toolchain",
                "manage installed toolchains",
                &[
                    planned("list", "list installed toolchains"),
                    planned("install", "install a toolchain"),
                    planned("prune", "remove unused toolchains"),
                ],
            ),
            planned("cache", "manage build caches"),
            planned("lsp", "run the language server"),
            Command {
                name: "components",
                about: "list the backends and linkers in this toolchain",
                params: &[],
                action: Action::Run(components),
            },
        ]),
    },
};

const FILE: Param =
    Param { name: "file", short: None, about: "the source file to read", kind: ParamKind::Positional };

const DUMPS: &[Command<Session>] = &[
    Command {
        name: "tokens",
        about: "tokens and trivia with byte ranges",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Tokens)),
    },
    Command {
        name: "cst",
        about: "the concrete syntax tree, one node per line",
        params: &[FILE],
        action: Action::Run(|s, args, out| dump(s, args, out, Stage::Cst)),
    },
    planned("hir", "the high-level representation"),
    planned("mir", "the mid-level representation"),
    planned("lir", "the low-level representation"),
    planned("link", "the link plan"),
];

const fn planned(name: &'static str, about: &'static str) -> Command<Session> {
    Command { name, about, params: &[], action: Action::Planned }
}

const fn group(
    name: &'static str,
    about: &'static str,
    children: &'static [Command<Session>],
) -> Command<Session> {
    Command { name, about, params: &[], action: Action::Group(children) }
}

/// What `overt dump` prints.
#[derive(Clone, Copy)]
enum Stage {
    Tokens,
    Cst,
}

fn dump(session: &mut Session, args: &Args, out: &mut dyn Write, stage: Stage) -> io::Result<Outcome> {
    let path = args.value(&FILE).expect("positional parameters are present when a command runs");
    let Some(file) = session.load(Path::new(path)) else { return Ok(Outcome::Failure) };
    let tokens = session.lex(file);
    match stage {
        Stage::Tokens => write!(out, "{}", tokens.dump(session.text(file)))?,
        Stage::Cst => {
            let cst = session.parse(&tokens);
            write!(out, "{}", cst.dump(&tokens, session.text(file)))?;
        }
    }
    Ok(if session.failed() { Outcome::Failure } else { Outcome::Success })
}

fn components(_: &mut Session, _: &Args, out: &mut dyn Write) -> io::Result<Outcome> {
    let r = Registry::with_defaults();
    let backends: Vec<_> = r.backends().map(|b| b.info().id).collect();
    let linkers: Vec<_> = r.linkers().map(|l| l.info().id).collect();
    writeln!(out, "backends: {}", backends.join(", "))?;
    writeln!(out, "linkers:  {}", linkers.join(", "))?;
    Ok(Outcome::Success)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn check_names(command: &Command<Session>) {
        let children = command.children();
        let names: HashSet<_> = children.iter().map(|c| c.name).collect();
        assert_eq!(names.len(), children.len(), "duplicate commands under {}", command.name);
        let params: HashSet<_> = command.params.iter().map(|p| p.name).collect();
        assert_eq!(params.len(), command.params.len(), "duplicate parameters of {}", command.name);
        children.iter().for_each(check_names);
    }

    #[test]
    fn names_are_unique_at_every_level() {
        check_names(&PROGRAM.root);
    }

    #[test]
    fn dump_cst_prints_the_tree() {
        let path = std::env::temp_dir().join(format!("overt-dump-{}.ov", std::process::id()));
        std::fs::write(&path, "x := 1;\n").unwrap();
        let argv = ["dump", "cst", path.to_str().unwrap()].map(str::to_owned);
        let (mut session, mut out) = (Session::new(), Vec::new());
        let outcome = overt_command::run(&PROGRAM, &argv, &mut session, &mut out).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(outcome, Outcome::Success);
        assert!(String::from_utf8(out).unwrap().starts_with("File Eof 8..8"));
    }
}
