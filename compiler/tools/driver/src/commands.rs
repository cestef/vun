//! The command set shared by every front end of the driver (spec 12, section 7).

pub struct Command {
    pub name: &'static str,
    pub about: &'static str,
}

pub const ALL: &[Command] = &[
    Command { name: "new", about: "create a package" },
    Command { name: "build", about: "build the package" },
    Command { name: "run", about: "build and run a program" },
    Command { name: "check", about: "analyze without generating code" },
    Command { name: "test", about: "run tests, properties and simulations" },
    Command { name: "bench", about: "run benchmarks" },
    Command { name: "fuzz", about: "fuzz a test with coverage guidance" },
    Command { name: "fmt", about: "format source files" },
    Command { name: "fix", about: "apply fixes from diagnostics" },
    Command { name: "doc", about: "generate documentation" },
    Command { name: "explain", about: "explain a diagnostic" },
    Command { name: "dump", about: "print an intermediate representation with origins" },
    Command { name: "pkg", about: "dependencies, releases and API diffs" },
    Command { name: "tool", about: "C bindings, C headers, register packages" },
    Command { name: "target", about: "flash, attach to and debug devices" },
    Command { name: "toolchain", about: "manage installed toolchains" },
    Command { name: "cache", about: "manage build caches" },
    Command { name: "lsp", about: "run the language server" },
    Command { name: "components", about: "list the backends and linkers in this toolchain" },
];

#[must_use]
pub fn find(name: &str) -> Option<&'static Command> {
    ALL.iter().find(|c| c.name == name)
}
