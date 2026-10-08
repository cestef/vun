# Commands

## Checks

```sh
cargo fmt && cargo clippy --all-targets && cargo nextest run && cargo gen --check && cargo xtask lines && cargo xtask layers
cargo xtask check             # the quick ones: formatting, generated sources, lines, layers
cargo xtask push -- <words>   # check, then `jj git push <words>`
cargo xtask hooks             # once per clone: the jj aliases `jj check` and `jj push <words>`
cargo xtask corpus            # verdicts, coverage, what stops MIR most
cargo xtask runtimes          # the std tests on every target this machine runs (its tools file: kernel, C library, WASI, wine)
cargo xtask tree-sitter       # build the tree-sitter parser (conflicts found), compare its trees with ours
cargo xtask install           # build `vun` (the dist profile) and install it as the default toolchain
cargo xtask install --profile dev   # the same, built quickly for development
VUN_QUERY_CHECK=1 cargo nextest run -p vun-driver   # after query, sema or MIR changes
VUN_BLESS=1 cargo nextest run -p vun-syntax         # snapshots, only after reading the diff
cd fuzz && cargo +nightly fuzz run parse -- -max_total_time=120   # after lexer or parser changes
cd fuzz && cargo +nightly fuzz run pe -- -max_len=65536       # after changes to the ELF or PE readers or the disassembler (targets elf, pe, disasm)
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

`tree-sitter-vun` is a repository of its own, published by `cargo xtask tree-sitter-publish
<message>`: it checks that the generated sources are current and that the parser agrees with the
compiler on every source (`cargo xtask tree-sitter`), copies what `vun_tree_sitter::Release::
PUBLISHED` names (`grammar.js`, `conflicts.json`, `tree-sitter.json`, `src/`, `queries/`) into the
clone, commits it there with the message and pushes `main` when anything changed, records the
commit in `editors/tree-sitter/release.json` and runs `cargo gen`: the Helix, Neovim and Zed
packages build that revision. The parser's `src/` is the tree-sitter CLI's own output, so publish
with the version the last publication used (`tree-sitter --version`: 0.27.0 on `am3.wg`, which
has `jj`; the clone is a path, `--repo`, `tree-sitter-vun` beside this repository by default).
The repository (`cestef/tree-sitter-vun`, private until the language is public, so the packages
fetch it only where git can authenticate) is a `jj` clone at `../tree-sitter-vun`; `jj` refuses
the generated `parser.c` (16 MiB) until `jj config set --repo snapshot.max-new-file-size 33554432`.
A warm `cargo xtask tree-sitter` takes about 10 s on `am3.wg`, nearly all of it one
single-threaded `tree-sitter generate`; each round settling the conflicts is one more.

## Releasing

A release is made by tagging the workspace's version, as it is (spec 12, section 6; decision
367): raise `version` in the root `Cargo.toml`, commit, then

```sh
jj tag set 0.1.0 -r @-
jj git push --tag 0.1.0
```

`.github/workflows/release.yml` runs the checks, builds `vun` in the `dist` profile on each
machine (`--no-default-features --features dist`: it links only what is not a component; Linux with
`cargo zigbuild` against glibc 2.28, dynamically linked so that `vun util ffi bind` can load libclang),
builds the component libraries (`cargo xtask components [--tool zigbuild] [--target <t>]`, from
every crate with a `[package.metadata.vun-component] profile`), packs it with
the library and each component beside it (`cargo xtask package <vun> <dir>`: `vun-<arch>-<os>.tar.gz`,
`vun-<kind>-<id>-<arch>-<os>.tar.gz` for each component, and the build's record beside it), writes `release.json` (`cargo xtask release <dir> --tag
<tag>`, which fails when the tag is not the version) and makes a draft release to publish by hand.
The same steps run on any machine; `VUN_RELEASES=file:///some/dir vun toolchain update` installs
from a directory laid out as a forge's (`releases/download/<version>/`,
`releases/latest/download/`), which is how installing is tried without a forge. `install.sh` at
the root installs the latest release for a first time; it checks the archive against the release's
signed `SHA256SUMS` first (`VUN_INSTALL_KEY` for a mirror's key). From a checkout instead of a release,
`cargo xtask install` builds `vun` and its components (the `dist` profile; `--profile release` or
`--profile dev` link everything in, for a faster build) and installs it as the toolchain of the workspace's version, replacing that
version and making it the default `~/.local/bin/vun` links to.

The release job signs `release.json` with the `RELEASE_KEY` secret (an Ed25519 key in PEM), and
`vun` checks it against `compiler/pkg/build/src/release/key.pub` (decision 368). The key is made,
or replaced, once, with its secret half kept off the repository:

```sh
bash -c 'set -e; d=~/.local/share/vun-release; mkdir -p "$d"; (umask 077; openssl genpkey -algorithm ed25519 -out "$d/key.pem")
openssl pkey -in "$d/key.pem" -pubout -outform DER | tail -c 32 | python3 -c "import sys, base64; print(\"ed25519:\" + base64.b32encode(sys.stdin.buffer.read()).decode().lower().rstrip(\"=\"))" > compiler/pkg/build/src/release/key.pub
gh secret set RELEASE_KEY < "$d/key.pem"'
```

A new key leaves toolchains built with the old one unable to check new releases, so it is
replaced only when the old one is lost or leaked. Installing from a directory signed with another
key: `openssl pkeyutl -sign -rawin -inkey key.pem -in release.json -out release.json.sig`, and
`VUN_RELEASES_KEY` naming its public half.

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
- Each needs `vun` on the path. It reads its library as spec 12, section 6 says: `VUN_LIBRARY`,
  else `lib/` beside its `bin/` (a release archive), else the checkout it was built from, so a
  binary copied alone to another host needs `VUN_LIBRARY` or the archive's layout.
  `VUN_LOG=vun_driver=debug` names the library read and why (`found=Named|Installed|Checkout`).
- The server can be driven without an editor: a JSON-RPC client over `vun util lsp`'s stdio
  (`initialize`, `didOpen`, then the requests) answers what an editor would be shown.

## The site and its playground

`site/` is a Vite project (pnpm): the landing page (`/`), the playground (`/play/`) and the course
(`/learn/`, lessons in `site/lessons/`, checked by `pnpm check:lessons`), which run the compiler
built for WASI in the browser. A page is a directory with an `index.html`, so addresses carry no
`.html`; the site's plugin redirects `/play` to `/play/` in the dev and preview servers as a static
host does, and serves the repository's `install.sh` at `/install.sh` (the landing page shows it
as download, read, run, never piped into a shell). Icons are one table (`src/icons.ts`, declared
in pages as `data-icon`) and keys are one table (`src/keys.ts`, `data-action` on buttons; `F1`
lists them). Build the compiler first, on a remote, and copy it back:

```sh
scripts/remote --host am3.wg cargo build -p vun-cli --target wasm32-wasip1 --profile dist
rsync am3.wg:vun-build/<workspace>/target/wasm32-wasip1/dist/vun.wasm target/wasm32-wasip1/dist/
cd site && pnpm install && pnpm dev    # or `pnpm build`, into `site/dist/`
vun doc --library -o site/public/api   # the API pages, served at /api/ (git-ignored; never `-o site`, which overwrites the landing page)
```

The site's plugin serves that file as `play/vun.wasm` and `lib/` as `play/library.json`. The
same compiler runs under wasmtime from a toolchain's tree (`tc/bin/vun` the module, `tc/lib/` the
library): `wasmtime run --dir .::/ --argv0 /tc/bin/vun tc/bin/vun build main.vun --target
wasi.wasm32.p1`. Its first argument names the executable, which WASI does not, so it finds its
library and keeps its memos (`.vun/`) as a native build does. It is 16 MB, 2.6 MB
with brotli. The language server waits on shared memory, which needs the page isolated across
origins: Vite's servers send the headers, and `site/public/_headers` asks a host that reads it
(Cloudflare Pages, Netlify); without them the editor has no server. `share.vun` is the preset
dictionary of share links and is never edited: a new one is a new file under the next version
letter in `share.ts`.

## Remote machines

```sh
scripts/remote --host am3.wg cargo nextest run   # or VUN_REMOTE=am3.wg
```

`scripts/remote` (a bash script, quoting each word as given) mirrors the working copy (rsync, without `/target` and `/.jj`) to
`vun-build/<workspace directory>` in the host's home and runs the words there over ssh, output
shown as it comes; each `jj` workspace has its own mirror and `target/`. A host needs rustup (the
toolchain file picks the version), `cargo-nextest`, `wasmtime`, `wild`, `clang`, `lld`, `mold` and,
to run cross targets, its own `~/.config/vun/tools.vun` (`am3.wg` has none: its cross tests skip, WASI programs among them, so the wasm linker's runs are tried on `petal.wg`). Both hosts have `wasm-tools` (`wasm-tools print`, `wasm-tools objdump`) for reading modules. The owner's: `am3.wg` (aarch64, 11 cores, the fastest;
x86-64-only driver tests skip there) and `petal.wg` (x86-64, 8 cores, with `qemu-user`, the arm64
cross compiler and the `fc44` sysroot, `arm-none-eabi-gcc` and `qemu-system-arm`; its tools file
has rows for `linux.arm64.kernel`, `linux.arm.kernel`, `bare.thumbv7em.eabihf` and `macos.arm64.darwin`). Known host differences: Fedora 44's libclang fails
`vun-clang`'s `a_header_reads_as_declarations_with_target_sizes` (`Elaborated` for `Record`), and
on the aarch64 host six driver tests of the native backend and the checks fail, since the machine's
own target there is `linux.arm64.kernel`.
The script builds nothing from the repository, so it works while the tree does not build; after a
`cargo gen` on a host, copy the generated directories back.
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
cargo run -q -p vun-cli -- debug disasm <elf> [symbol]   # an ELF file's code as instructions (yaxpeax-x86, yaxpeax-arm)
cargo run -q -p vun-cli -- debug coverage|compare <file>
cargo run -q -p vun-cli -- test [path] [names]...   # the tests of a namespace, as std.testing.runner runs them
cargo run -q -p vun-cli -- test [path] --jobs 4      # each test in a process of its own, 4 at a time (cores by default; 1 = one process, in order)
cargo run -q -p vun-cli -- test <path> <name> --seed <n> [--cases <n>]   # replay a falsified property's cases
cargo run -q -p vun-cli -- bench [--release] [path] [names]... [--samples n] [--measure instructions] [--save] [--fail-over 3%]   # the benchmarks alone, measured over samples, against a baseline in .vun/bench/
cargo run -q -p vun-cli -- fuzz [path] [names]... [--cases n]   # the properties alone, fuzzed, built --cover
cargo run -q -p vun-cli -- cover [path] [names]... [--functions] [--lcov file]   # lines and functions the tests reach, built --cover
cargo run -q -p vun-cli -- debug smith --count 30 --seed 0 --reach run --out <dir> [--reduce] [--jobs 5] [--warm]   # programs made by chance, tried through every stage and backend
cargo run -q -p vun-cli -- debug queries <file>   # what the saved queries hold, per query and pool
cargo run -q -p vun-cli -- debug compare --target linux.arm64.kernel <file>   # every backend, through qemu
cargo run -q -p vun-cli -- debug compare --runs 20 <file>   # run time as the median of 20 runs
cargo run -q -p vun-cli -- toolchain show        # version, library and why there, how many components
cargo build -p vun-component-backend-cranelift --features vun-component-backend-cranelift/export   # a component as a library: target/<profile>/libvun_component_backend_cranelift.so
VUN_COMPONENTS=target/debug vun component list  # components loaded from that directory, beside the built-in ones
vun component add|remove <name>...               # install from the toolchain's release (its kept manifest), or take away
cargo run -q -p vun-cli -- target list           # every target: backend, linker, where it runs, what is lacking
cargo run -q -p vun-cli -- target show --target linux.arm64.kernel   # each part of a build and where it comes from
```

`vun toolchain prime` checks the libraries whole and keeps their shared memos in the running
toolchain's `memos/` (`--into <dir>` elsewhere): about 5 s and 75 MB on `am3.wg`, 10 to 13 MB in a
release archive. `cargo xtask package` and `install` run it on the staged executable; a build in
`target/` has no `memos/` and primes its own shared files on first use. The compiler is named by
the hash of its executable, remembered in `<store>/builds`, so the first run of a new build reads
the whole file (a debug build is 300 MB: a few hundred milliseconds); measure from a store whose
`builds` is kept and `memos` is emptied, or the first run is the hash. To try a primed toolchain
where it was not made, copy `bin/vun`, `lib/` and `memos/` to a new directory and run it: the
memos hold wherever the tree is.

Saved queries live in `.vun/queries/` of the project (its package's directory, a workspace's root, or beside a file outside any package; delete it to start cold), builds in
`.vun/build/`. The libraries' memos are shared entries of the store (`<store>/memos`, per
toolchain, target and checks).

`cargo xtask runtimes` runs `vun test --target T lib/std` for each target `vun target list` says runs
on this machine, one line each. The runtimes are code of their own (`std.os.linux.kernel`,
`std.os.libc`, `std.os.wasi`, `std.os.windows`), so a machine runs what it has the tools for: its
own kernel and C library targets, and, through `vun/tools.vun` (`.runner`), WASI under `wasmtime`
(`.runner = .{ "wasmtime", "run" }`), Windows under `wine64` with a `.sysroot` holding `kernel32.lib`
(zig's cache has one), Arm under `qemu-aarch64`. Run it on each host with
`scripts/remote --host am3.wg cargo xtask runtimes`; am3 (arm64) covers `linux.arm64.kernel`,
`linux.arm64.gnu` and WASI, petal (x86-64) `linux.x86_64.kernel`, `linux.x86_64.gnu` and WASI, the
laptop adds Windows under wine. macOS has no machine here.

`vun debug compare` is a table with a row per backend (exit, bytes written to each stream, program size,
time to generate and link, median run time) and, for each backend that parts from the first, the first line
of stdout or stderr (or the exit code) where it does; `--format json` gives a `backend` line per row, a
`parting` line per difference and a `verdict`. The exit status fails when any backend differs.

## Logging

Progress bars (`vun_term::Progress`): open one with `shell.bar(..)` in a command or
`Progress::ambient().bar(..)` in library code, advance it from any thread, drop it when done.
A program that writes to the terminal itself (`vun run`) calls `Session::foreground()` first.
Check the drawing on a terminal with a pty (`script`, or Python's `pty.fork`), not a pipe.

`VUN_LOG=<filter>` (`info` a step of a run, `debug` a unit of work, `trace` an element),
`VUN_LOG_FORMAT=tree`; `-vv` is `VUN_LOG=info`, `-vvv` `debug`, `-vvvv` `trace` (`VUN_LOG` wins). Logs and
progress bars tear each other, so bars are not drawn while logging. Useful filters:

| Filter | Shows |
|--------|-------|
| `vun_mir=debug,vun_lower=trace` | building and lowering statement by statement, naming what fails |
| `vun_syntax=trace` | each decision of the parser at each token (who reads it, a form continuing an operand, an end, a mend), each node read with the kinds it spells, each report and whether it was fallout |
| `vun_sema_asm=trace` | each form an assembly line was tried against and why it missed |
| `vun_sema_typeck=trace` | each compile-time evaluation while typing that gave no value, where and why (`Runtime`, `Unsupported`, ...) |
| `vun_codegen=trace` | each instruction selected |
| `vun_lower::assemble=debug,trace` | each function the program holds (trace) and each callee it lacks (debug), as `file#expr bound [x_]` |
| `vun_backend_wasm=debug,trace` | each wasm function lowered, each instruction's code |
| `vun_backend_cranelift=debug,trace` | each Cranelift function compiled or handed to the assisting backend, each instruction lowered (`vun debug dump code --backend cranelift` prints the IR and listing with origins) |
| `vun_component=debug` | each call into a loaded component (bytes each way, microseconds, the library), each component opened; `warn`: a library refused (other release or layout) |
| `vun_link_lld=debug` | each `wasm-ld` run |
| `vun_link_wasm=debug` | each wasm link in process |
| `vun_lir_passes=debug` | each function's tidy (`trace`: each pass) |
| `vun_query=debug` | each query computed and why, shared memos named |
| `vun_query::cx=debug,vun_sema=debug` | each compile-time call under a `call{ f="name" }` span of the call that made it: count the lines of `computing query="metered_call"` by their innermost (callee's caller) or outermost span to see what the interpreter runs |
| `vun_query::changed=debug` | each persisted memo computed anew, with why it was asked and how it differs from the one held; cheap alone, so it does not hide a race (`Db::first_changed` names the first) |

## Cross builds

Cross builds read the machine's tools file (spec 12 section 6, decisions 192 and 366),
`~/.config/vun/tools.vun`; `VUN_TOOLS` points at another file; `vun target show` names the entry
each part comes from. Every session that chooses
a target reads it, tests included: a broken file, or an entry naming a target the branch lacks,
fails every test choosing a target (`target.unchosen`). Without a row for a target, the backend
tests skip it.

On the owner's machine:

| Target | Compiler | Runner |
|--------|----------|--------|
| `linux.arm64.kernel` | `aarch64-linux-gnu-gcc`, sysroot `/usr/aarch64-redhat-linux/sys-root/fc43` | `qemu-aarch64 -L <sysroot>/usr` |
| `linux.arm.kernel` | `clang --target=arm-linux-gnueabi -fuse-ld=lld` (finds the `libgcc` of `gcc-arm-linux-gnu`) | `qemu-arm` |
| `bare.thumbv7em.eabihf` | the target's `arm-none-eabi-gcc` (`arm-none-eabi-gcc-cs`) | `qemu-system-arm -M netduinoplus2 -nographic -semihosting-config enable=on,target=native -kernel` |
| `macos.arm64.darwin` | the target's `clang`, sysroot `~/sdk/MacOSX.sdk` (links with `ld64.lld`) | none: programs are linked, not run |

The macOS SDK on `petal.wg` is assembled from Zig's own (`dnf install zig`, MIT licensed), not
Apple's, which is licensed for Apple hardware only: `SDKSettings.json` (with `Version` and
`SupportedTargets`, or clang ignores it) and `usr/lib/libSystem.tbd` from
`/usr/lib/zig/libc/darwin`, and `usr/include` from `/usr/lib/zig/libc/include/any-darwin-any`.
It links; no machine here runs what it links.

GCC refuses `r7` in the Thumb runtime's blocks (its Thumb frame pointer), hence clang for
`linux.arm.kernel`. Kernel targets link `-static -nostdlib -l gcc`.

## Cargo features

`vun-cli` builds what its features name; the default has all of them. `backends` and `linkers`
group the code generators and linkers (each is also a feature: `backend-llvm`, `link-wild`, ...),
`native-x86_64`, `native-arm64` and `native-thumb` the architectures of the native backend,
`ffi` is `vun util ffi bind` (libclang) linked in (the release has it as a library), `lsp` is `vun util lsp` linked in (the release has it as a library), `mimalloc` the allocator. A build
without a component reports it where it is asked for: no backend or linker of that name, `util
lsp` unavailable. `cargo check -p vun-cli --no-default-features` is the smallest toolchain, and
`--features backend-c,link-gnu` adds to it. `vun-driver` has the same features with no default
for its dependents, so a dependent chooses (`xtask` and `fuzz` take them all).

Language features are another thing: a package's `.features` (spec 12, section 2).

## Measuring speed

```sh
cargo bench -p vun-syntax --bench syntax -- --save-baseline main   # lex and parse lib/ and the corpus
cargo bench -p vun-driver --bench compile -- --baseline main       # compared against a baseline
vun check --timings --timeline /tmp/t.json file.vun                # open t.json in ui.perfetto.dev
```

- Benchmarks (criterion, `benches/` of `vun-syntax` and `vun-driver`): `syntax/{lex,parse}` over
  every source; `cold/{check,code}/<program>` from nothing, `saved/check/<program>` from the memos
  an earlier run saved, `edit/check/<program>` after appending a declaration in one process, for
  `hello`, `tour` and `threads`. Run them on a remote (`scripts/remote --host am3.wg cargo
  bench ...`); `--save-baseline <name>` before a change, `--baseline <name>` after. A filter
  after `--` runs some (`-- cold/check`). `cargo bench` builds with the release profile.
- `--timings` (or `VUN_TIMINGS=1`) reports each stage and query's own and total time and its
  longest run, with how work spread over threads; `--memos` (or `VUN_MEMOS=1`) what each query
  did with its memos. Each call of a component is a `component <kind>` row whose own time is
  the boundary's cost (encoding, copying, decoding); what a library times on the thread of the
  call shows under it, as stages and queries do.
- `--timeline <file>` (or `VUN_TIMELINE=<file>`) writes every run of each stage and query on each
  thread, for Perfetto or `chrome://tracing`: which runs overlapped, how busy the workers were,
  what the main thread waited on. Gaps on the main thread are untimed work: time it as a stage.
- A profile of a run, on a remote with `perf`: `perf record -F 4999 -g --call-graph=fp -o
  /tmp/v.perf target/release/vun check f.vun`, then `perf report -i /tmp/v.perf --no-children
  --sort symbol --stdio -g none`. A benchmark alone: `target/release/deps/<bench>-<hash> --bench
  --profile-time 5 <filter>` under `perf record`. Bound every run with `timeout`.
- `vun debug profile [workload...]` measures the compiler and the programs it builds so that two
  runs compare. `--measure instructions` (the default) runs under `valgrind --tool=callgrind`
  with one thread: the count of a warm run repeats to under 0.1%, so a change of 0.5% is real;
  it ranks crates (`Source`) and functions, with calls and, where the function is not in a call
  cycle, the cost with its callees (`cycle` otherwise; most of the compiler is one cycle through
  the query engine), and `--callers` lists who calls each. `--measure time` takes `--samples`
  runs of `--timings` (cold from an empty `VUN_STORE` each time, warm after one unmeasured run)
  and ranks stages and queries by own time with the spread. Workloads are `check-*` and
  `build-*` (the compiler, `--phase cold|warm`, both by default) and `run-*` (`corpus/bench`: the
  program is built unmeasured, then the program is counted or timed; `--backend` picks the code
  generator, release profile). `--save` keeps the results as a snapshot under `.vun/profile/`,
  and a later run prints the change against it (`--snapshot <name>`, `--fail-over <pct>` fails
  on a regression). Cold counts differ by about 1% between runs, so judge a change on the warm
  phase or on time. Names need debug information: build the measured compiler with
  `RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_RELEASE_DEBUG=1
  CARGO_TARGET_DIR=target/profiling cargo build --release -p vun-cli` and pass it as `--exe`.
  Run it on a remote with `valgrind` (`am3.wg`). Callgrind stops on an instruction it does not
  know (a cranelift-built `sieve` raises SIGILL under it and not outside); use `--measure time`.
- Against other languages: `scripts/remote --host am3.wg scripts/bench-compare` builds the programs of
  `corpus/bench` and their ports (`corpus/bench/ref/<language>/`: C, Rust, Zig, Go, Hare) and writes
  `assets/bench/results.json`; `cargo xtask typeset` draws it as the charts of `assets/cards/bench`
  (the README) and of the whitepaper's implementation section. The ports use the same algorithm and
  constants with wrapping arithmetic, and every executable's output is compared with a reference.
  Each is timed as a process, pinned to the fastest core, over `--samples` runs after warm-up, with
  its optimized and its unoptimized build both reported; compile times are fresh builds, those that keep a cache (Vun,
  Zig, Go, Hare) from empty caches (cold) and rebuilt with the caches a build filled (warm). Zig, Go and Hare are unpacked into
  `~/bench-tools` (`--tools`): Zig from its release tarball, Go and `harec` from the Fedora
  packages, Hare's standard library built from the tag that matches `harec`.
- Memory per nesting level of compile-time calls: evaluate a recursion through an annotation
  (`depth := (n: u32) -> u32 { if n == 0 { 0 } else { (1 + depth(n - 1)).sat(u32) } };
  c: Int(depth(N)..=depth(N)) = 1;`) for two depths, each from nothing (`rm -rf .vun`,
  `VUN_STORE=$(mktemp -d)`), under `/usr/bin/time -f "%M KB"`, and divide the difference of the
  peaks by the difference of depths: 8 KB a level in the dev profile (it was 12). The heap shows
  in `valgrind --tool=massif` of a build without the `mimalloc` feature (`cargo build -p vun-cli
  --no-default-features`), the stack in the frame sizes of the calls one level goes through
  (`objdump -d -C` and the first `sub sp, sp, #n` of `Frame::call`, `Frame::execute`,
  `Cx::computed`, ...). A `Value` is held in each level's arguments, locals and memos, so its size
  is the lever (`vun-sema-ty`'s tests guard it).
- A profile on the laptop with [stax](https://stax.bearcove.eu) (`stax record -- <binary> <args>`,
  then `stax top --sort self|total` and `stax flame -d <depth>`; needs `staxd`, `stax-server` and
  `kernel.perf_event_mlock_kb=65536`): build the binary apart, `RUSTFLAGS="-C
  force-frame-pointers=yes" CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_TARGET_DIR=target/profiling cargo
  build --release -p vun-cli`, without frame pointers stacks stop after a few frames. Profile a run
  from the second one on: the first computes and saves the libraries' memos, which costs more than
  a build of a small program. `hello` is too short to sample; use `vun check lib/std`.
- The language server (`vun util lsp`) times each message as a stage named by its method, and
  `publish diagnostics`; `VUN_LOG=vun_lsp=debug` logs each message handled with how long it
  took, in the editor's server log.
- `VUN_THREADS=1` gives exact, repeatable counts.
- `VUN_STORE` points the store elsewhere, isolating a measurement from the shared memos.
  Sessions of another toolchain remove this one's shared files.
- The CPU clock varies (a laptop on battery): compare A/B runs made together, never across time.
  `am3.wg` runs other services on mixed cores: runs spread over threads show outliers of tens of
  milliseconds there when a worker is preempted; measure with `scripts/remote --reserve`,
  compare medians, or `VUN_THREADS=1`.
