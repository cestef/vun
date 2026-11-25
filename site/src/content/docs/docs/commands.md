---
title: "Commands"
description: "Every command of `vun`, by what it does."
---

`vun` is one binary. Everyday verbs sit at the root, nouns are groups of verbs (`vun pkg add`), the rarely used ones sit under `util`, and the compiler's own tools under `debug`. `vun help <command>` and `--help` print the details.

## Everyday

| Command | Does |
|---|---|
| `new <path>` | start a project from a template |
| `run [path] [args]` | build the program a root file or package exports, then run it |
| `build [path]` | build it without running |
| `check [path]` | analyze without generating code |
| `test [path] [names]` | run the tests of a namespace |
| `fmt [paths]` | lay out source files as the formatter does |
| `fix [path]` | apply the fixes diagnostics propose |
| `explain <id>` | explain a diagnostic |
| `doc [packages]` | write the API documentation of packages |
| `bench`, `fuzz`, `cover` | benchmarks, property fuzzing guided by coverage, line coverage of the tests |
| `flash` | build a package's programs and write those it flashes to their devices |

`run`, `build`, `check`, `test` and `fix` take a file, a directory or a package's directory. With none they use the package around the current directory.

## Groups

| Group | Does |
|---|---|
| `pkg` | `add` a dependency, `update` them, `pack` the package's archive |
| `target` | `list` the targets and `show` what a build for one uses |
| `toolchain` | `show`, `list`, `install`, `update`, `default`, `remove`, `prime` |
| `component` | the backends, linkers and other parts of a toolchain, and which one has |
| `util` | `completion`, `lsp`, `cache`, `ffi` |
| `debug` | dumps of every representation, coverage of the stages, comparing backends |

## Options you will use

Build commands (`run`, `build`, `check`, `test`) share these.

| Option | Does |
|---|---|
| `-r`, `--release` | build fast code rather than build fast |
| `--target <target>` | the target to compile for (see [Targets](../targets/)) |
| `--backend <backend>` | `qbe`, `llvm`, `external`, `c`, `cranelift`, `native` or `wasm` |
| `--linker <linker>` | `msvc`, `external`, `ar`, `gnu`, `lld`, `mold`, `wasm` or `wild` |
| `--lint <lint>` | raise or lower a lint or a group: `--lint std.lints.naming.case=warn` |
| `--checks <checks>` | what to check unsafe code for at run time: `pointers`, `allocations`, `asm`, `stack` |
| `--trace <trace>` | what a trap reports: `off`, `sites` or `calls` |
| `--timings` | the time of each stage and query, at exit |

Every command also takes these:

| Option | Does |
|---|---|
| `--format <format>` | `human`, `styled`, or `json` for JSON lines a program can read |
| `--color <color>` | `auto`, `always` or `never` |
| `-q`, `--quiet` | results and diagnostics, without status lines |
| `-v`, `--verbose` | each step; `-vv` adds the compiler's log |

Most options also read an environment variable (`VUN_TARGET`, `VUN_BACKEND`, `VUN_FORMAT`, `VUN_COLOR`).

## Output for programs

```sh
vun check --format json
```

Each record the command reports (a status line, a table row, a diagnostic) comes out as one line of JSON, so an editor, a CI job or a script reads what a person would. Diagnostics alone can also be rendered as `short`, `ariadne`, `sarif` or `terminal` with `--diagnostics`.

## Shell completion

```sh
vun util completion --help
```

prints the script for your shell.
