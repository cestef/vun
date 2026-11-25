---
title: "Toolchains and editors"
description: "Install and switch releases, and connect an editor."
---

## Toolchains

A toolchain is one release of `vun` with the library and components that go with it. Several can be installed side by side, and one is the default that `~/.local/bin/vun` runs.

```sh
vun toolchain list
```

```ansi
[1m[32mversion[0m                    [1m[32mdir[0m
[1m[36m0.2.0[0m    [1m[32mdefault, running[0m  [2m~/.local/share/vun/toolchains/0.2.0[0m
```

| Command | Does |
|---|---|
| `toolchain show` | the running toolchain: its version, binary, library and components |
| `toolchain list` | the installed toolchains, the running one and the default marked |
| `toolchain install [version]` | install a release beside the others (the latest when none) |
| `toolchain update` | install the latest release and make it the default |
| `toolchain default <version>` | make an installed toolchain the default |
| `toolchain remove <version>` | remove one |
| `toolchain prime` | check the libraries whole and ship their memos with the toolchain, so the first check finds them |

`install` takes `--profile minimal|default|full` for how many of the release's components come with it, and `--default` to make it the default.

## Components

The backends, linkers and other parts a build can use are components of a toolchain, and `vun component` shows which one has. A target that lacks one says so in `vun target list`.

## Editors

`vun util lsp` serves the language server over standard input and output. Editors start it for you:

| Editor | Where |
|---|---|
| VS Code | `editors/vscode` |
| Zed | `editors/zed` (a dev extension) |
| Helix | `editors/helix` (`languages.toml` and queries) |
| Neovim | `editors/nvim` |
| Sublime Text | `editors/sublime` (syntax only) |

Every editor directory is in the repository. The tree-sitter grammar in `editors/tree-sitter` is generated from the language's grammar sources.

The server is built and tested but has seen little use in real editors. Report what breaks.

## Environment

| Variable | Does |
|---|---|
| `VUN_LOG=<filter>` | log from the compiler: `info` a line per step, `debug` per unit of work, `trace` per statement |
| `VUN_TARGET`, `VUN_BACKEND`, `VUN_LINKER` | defaults for the matching options |
| `VUN_FORMAT`, `VUN_COLOR` | how output is shown |
| `VUN_LIBRARY` | the library directory, for a binary copied away from its toolchain |
