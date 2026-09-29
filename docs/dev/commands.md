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
cross compiler and the `fc44` sysroot). Known host differences: Fedora 44's libclang fails
`vun-clang`'s `a_header_reads_as_declarations_with_target_sizes` (`Elaborated` for `Record`), and
on the aarch64 host six driver tests of the native backend and the checks fail, since the machine's
own target there is `linux.arm64`.
`cargo xtask tree-sitter` needs the `tree-sitter` CLI (`cargo binstall tree-sitter-cli`); on a host
without `node`, `TREE_SITTER_JS_RUNTIME=native` loads `grammar.js` with its built-in QuickJS.

## Running and inspecting

```sh
cargo run -q -p vun-cli -- run|build [--release] [--backend b] [--target t] [--trace off|sites|calls] <file>
cargo run -q -p vun-cli -- debug tokens|cst|manifest|plan|hir|types|mir|layout|program|lir|code|frames|coverage|compare <file>
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
| `vun_sema::asm=trace` | each form an assembly line was tried against and why it missed |
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

GCC refuses `r7` in the Thumb runtime's blocks (its Thumb frame pointer), hence clang for
`linux.arm`. Kernel targets link `-static -nostdlib -l gcc`.

## Measuring speed

- `--timings` (or `VUN_TIMINGS=1`) reports each stage and query's own and total time, with how
  work spread over threads; `--memos` (or `VUN_MEMOS=1`) what each query did with its memos.
- `VUN_THREADS=1` gives exact, repeatable counts.
- `VUN_STORE` points the store elsewhere, isolating a measurement from the shared memos.
  Sessions of another toolchain remove this one's shared files.
- The CPU clock varies (a laptop on battery): compare A/B runs made together, never across time.
