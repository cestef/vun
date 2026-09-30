# Commands

## Checks

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines && cargo xtask layers
cargo xtask check             # the quick ones: formatting, generated sources, lines, layers
cargo xtask push -- <words>   # check, then `jj git push <words>`
cargo xtask hooks             # once per clone: the jj aliases `jj check` and `jj push <words>`
cargo xtask corpus            # verdicts, coverage, what stops MIR most
cargo xtask tree-sitter       # build the tree-sitter parser (conflicts found), compare its trees with ours
cargo xtask size              # the distributed binary against its budget (20 MiB)
VUN_QUERY_CHECK=1 cargo nextest run -p vun-driver   # after query, sema or MIR changes
VUN_BLESS=1 cargo nextest run -p vun-syntax         # snapshots, only after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120   # after lexer or parser changes
cargo xtask typeset           # whitepaper PDF and README card, with typst; reads the generated editor grammar
```

`VUN_QUERY_CHECK=1` (or `Db::reusing(Reuse::Check)`) computes every reused memo again and panics
with the query and key when it differs (`Unsound`). The driver tests share the libraries' memos
between sessions (`target/tmp/memos`) and do not check them by default, so run it whenever a
change could make a saved query wrong.

Changes to `vun-par`'s pool run its Miri tests:

```sh
MIRIFLAGS="-Zmiri-num-cpus=4 -Zmiri-many-seeds=0..8 -Zmiri-ignore-leaks" cargo +nightly miri test -p vun-par --lib
```

## Publishing the tree-sitter grammar

`tree-sitter-vun` is a repository of its own, published by hand: copy `grammar.js`,
`conflicts.json`, `tree-sitter.json`, `src/` (after `cargo xtask tree-sitter`) and `queries/` from
`editors/tree-sitter` into it and commit there. Then record the commit beside the grammar,
`editors/tree-sitter/release.json` as `{ "revision": "<commit>" }`, and run `cargo gen`: the Helix,
Neovim and Zed packages build that revision (the branch `main` while no release is recorded).
The repository (`cestef/tree-sitter-vun`, private until the language is public, so the packages
fetch it only where git can authenticate) is a `jj` clone at `../tree-sitter-vun`; `jj` refuses
the generated `parser.c` (16 MiB) until `jj config set --repo snapshot.max-new-file-size 33554432`.
A warm `cargo xtask tree-sitter` takes about 10 s on `am3.wg`, nearly all of it one
single-threaded `tree-sitter generate`; each round settling the conflicts is one more.

## Trying the editor packages

- Helix: append `editors/helix/languages.toml` to `~/.config/helix/languages.toml`, link
  `editors/helix/queries/vun` into `~/.config/helix/runtime/queries/`, then
  `hx --grammar fetch && hx --grammar build` (or build it under a scratch `XDG_CONFIG_HOME` with
  `use-grammars = { only = ["vun"] }` and copy `grammars/vun.so`); `hx --health vun` checks it.
- Neovim 0.11: nvim-treesitter `master` (its `main` needs 0.12), the `editors/nvim` directory on
  the runtime path, `require("nvim-treesitter.install").prefer_git = true` while the grammar's
  repository is private, then `:TSInstall vun`.
- VS Code or VSCodium: `npm install` in `editors/vscode`, `npx @vscode/vsce package
  --allow-missing-repository --skip-license -o vun.vsix`, `codium --install-extension vun.vsix`.
- Zed: `zed: install dev extension` on `editors/zed`, which Zed compiles for `wasm32-wasip2`.
- Each needs `vun` on the path: `LIBRARY` is the checkout's `lib/` as the build saw it, so a
  binary built elsewhere is built at the same path (`rsync` the checkout to that path on the
  host).
- The server can be driven without an editor: a JSON-RPC client over `vun util lsp`'s stdio
  (`initialize`, `didOpen`, then the requests) answers what an editor would be shown.

## Remote machines

```sh
cargo xtask remote --host am3.wg -- cargo nextest run   # or VUN_REMOTE=am3.wg
```

`remote` mirrors the working copy (rsync, without `/target` and `/.jj`) to
`vun-build/<workspace directory>` in the host's home and runs the words there over ssh, output
shown as it comes; each `jj` workspace has its own mirror and `target/`. A host needs rustup (the
toolchain file picks the version), `cargo-nextest`, `wasmtime`, `wild`, `clang`, `lld`, `mold` and
its own `~/.config/vun/toolchain.vun`. The owner's: `am3.wg` (aarch64, 11 cores, the fastest;
x86-64-only driver tests skip there) and `petal.wg` (x86-64, 8 cores, with `qemu-user`, the arm64
cross compiler and the `fc44` sysroot, `arm-none-eabi-gcc` and `qemu-system-arm`; its toolchain file
has rows for `linux.arm64`, `linux.arm`, `bare.cortex_m4f` and `macos.arm64`). Known host differences: Fedora 44's libclang fails
`vun-clang`'s `a_header_reads_as_declarations_with_target_sizes` (`Elaborated` for `Record`), and
on the aarch64 host six driver tests of the native backend and the checks fail, since the machine's
own target there is `linux.arm64`.
`--reserve` runs the words on the host's fastest cores, those of the highest `cpu_capacity`
(`am3.wg`'s 0 and 7 to 10), with the host's services (`system.slice`) confined to the others and
given them back after, by `systemctl set-property --runtime` (a reboot undoes it too); it needs
passwordless `sudo` on the host and refuses a host whose cores are all alike. Use it for
benchmarks and timings.
`cargo xtask tree-sitter` needs the `tree-sitter` CLI (`cargo binstall tree-sitter-cli`); on a host
without `node`, `TREE_SITTER_JS_RUNTIME=native` loads `grammar.js` with its built-in QuickJS.

## Running and inspecting

```sh
cargo run -q -p vun-cli -- run|build [--release] [--backend b] [--target t] [--trace off|sites|calls] <file>
cargo run -q -p vun-cli -- debug dump tokens|cst|manifest|plan|hir|types|values|mir|layout|program|lir|code|frames <file>
cargo run -q -p vun-cli -- debug coverage|compare <file>
cargo run -q -p vun-cli -- debug queries <file>   # what the saved queries hold, per query and pool
cargo run -q -p vun-cli -- debug compare --target linux.arm64 <file>   # every backend, through qemu
cargo run -q -p vun-cli -- toolchain components   # each component's status and what it handles
```

Saved queries live in `.vun/queries/` beside the root file (delete it to start cold), builds in
`.vun/build/`. The libraries' memos are shared entries of the store (`<store>/memos`, per
toolchain, target and checks).

## Logging

`VUN_LOG=<filter>` (`info` a step of a run, `debug` a unit of work, `trace` an element),
`VUN_LOG_FORMAT=tree`. Useful filters:

| Filter | Shows |
|--------|-------|
| `vun_mir=debug,vun_lower=trace` | building and lowering statement by statement, naming what fails |
| `vun_syntax=trace` | each decision of the parser at each token (who reads it, a form continuing an operand, an end, a mend), each node read with the kinds it spells, each report and whether it was fallout |
| `vun_sema::asm=trace` | each form an assembly line was tried against and why it missed |
| `vun_sema::typeck=trace` | each compile-time evaluation while typing that gave no value, where and why (`Runtime`, `Unsupported`, ...) |
| `vun_codegen=trace` | each instruction selected |
| `vun_backend_wasm=debug,trace` | each wasm function lowered, each instruction's code |
| `vun_link_lld=debug` | each `wasm-ld` run |
| `vun_lir_passes=debug` | each function's tidy (`trace`: each pass) |
| `vun_query=debug` | each query computed and why, shared memos named |

## Cross builds

Cross builds read the machine's toolchain file (spec 12 section 6, decision 192),
`~/.config/vun/toolchain.vun`; `VUN_TOOLCHAIN` points at another file. Every session that chooses
a target reads it, tests included: a broken file, or an entry naming a target the branch lacks,
fails every test choosing a target (`target.unchosen`). Without a row for a target, the backend
tests skip it.

On the owner's machine:

| Target | Compiler | Runner |
|--------|----------|--------|
| `linux.arm64` | `aarch64-linux-gnu-gcc`, sysroot `/usr/aarch64-redhat-linux/sys-root/fc43` | `qemu-aarch64 -L <sysroot>/usr` |
| `linux.arm` | `clang --target=arm-linux-gnueabi -fuse-ld=lld` (finds the `libgcc` of `gcc-arm-linux-gnu`) | `qemu-arm` |
| `bare.cortex_m4f` | the target's `arm-none-eabi-gcc` (`arm-none-eabi-gcc-cs`) | `qemu-system-arm -M netduinoplus2 -nographic -semihosting-config enable=on,target=native -kernel` |
| `macos.arm64` | the target's `clang`, sysroot `~/sdk/MacOSX.sdk` (links with `ld64.lld`) | none: programs are linked, not run |

The macOS SDK on `petal.wg` is assembled from Zig's own (`dnf install zig`, MIT licensed), not
Apple's, which is licensed for Apple hardware only: `SDKSettings.json` (with `Version` and
`SupportedTargets`, or clang ignores it) and `usr/lib/libSystem.tbd` from
`/usr/lib/zig/libc/darwin`, and `usr/include` from `/usr/lib/zig/libc/include/any-darwin-any`.
It links; no machine here runs what it links.

GCC refuses `r7` in the Thumb runtime's blocks (its Thumb frame pointer), hence clang for
`linux.arm`. Kernel targets link `-static -nostdlib -l gcc`.

## Measuring speed

```sh
cargo bench -p vun-syntax --bench syntax -- --save-baseline main   # lex and parse lib/ and the corpus
cargo bench -p vun-driver --bench compile -- --baseline main       # compared against a baseline
vun check --timings --timeline /tmp/t.json file.vun                # open t.json in ui.perfetto.dev
```

- Benchmarks (criterion, `benches/` of `vun-syntax` and `vun-driver`): `syntax/{lex,parse}` over
  every source; `cold/{check,code}/<program>` from nothing, `saved/check/<program>` from the memos
  an earlier run saved, `edit/check/<program>` after appending a declaration in one process, for
  `hello`, `tour` and `threads`. Run them on a remote (`cargo xtask remote --host am3.wg -- cargo
  bench ...`); `--save-baseline <name>` before a change, `--baseline <name>` after. A filter
  after `--` runs some (`-- cold/check`). `cargo bench` builds with the release profile.
- `--timings` (or `VUN_TIMINGS=1`) reports each stage and query's own and total time and its
  longest run, with how work spread over threads; `--memos` (or `VUN_MEMOS=1`) what each query
  did with its memos.
- `--timeline <file>` (or `VUN_TIMELINE=<file>`) writes every run of each stage and query on each
  thread, for Perfetto or `chrome://tracing`: which runs overlapped, how busy the workers were,
  what the main thread waited on. Gaps on the main thread are untimed work: time it as a stage.
- A profile of a run, on a remote with `perf`: `perf record -F 4999 -g --call-graph=fp -o
  /tmp/v.perf target/release/vun check f.vun`, then `perf report -i /tmp/v.perf --no-children
  --sort symbol --stdio -g none`. A benchmark alone: `target/release/deps/<bench>-<hash> --bench
  --profile-time 5 <filter>` under `perf record`. Bound every run with `timeout`.
- The language server (`vun util lsp`) times each message as a stage named by its method, and
  `publish diagnostics`; `VUN_LOG=vun_lsp=debug` logs each message handled with how long it
  took, in the editor's server log.
- `VUN_THREADS=1` gives exact, repeatable counts.
- `VUN_STORE` points the store elsewhere, isolating a measurement from the shared memos.
  Sessions of another toolchain remove this one's shared files.
- The CPU clock varies (a laptop on battery): compare A/B runs made together, never across time.
  `am3.wg` runs other services on mixed cores: runs spread over threads show outliers of tens of
  milliseconds there when a worker is preempted; measure with `cargo xtask remote --reserve`,
  compare medians, or `VUN_THREADS=1`.
