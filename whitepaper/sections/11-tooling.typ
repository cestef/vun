#import "../template.typ": *

= Diagnostics, testing and packages <sec-tooling>

== Provenance

Every entity in every stage of the compiler holds an origin identifier into a shared table. An
origin records a source span, the way the entity came to exist, and its parent origin. The ways
are: written in the source, produced by a desugaring rule, instantiated from a generic body,
spliced from a quotation or template, named by a computed name, or introduced by a lowering pass.
Chains are shared, so the cost is one 32-bit index per entity, and an entity without an origin is
rejected by the verifiers that run between passes. The same chains produce debug line tables,
`#line` directives in generated C, the traces attached to diagnostics in generated code, and the
source locations reported for run-time traps.

Origins have stable identities so that they can be used in persistent caches. A file is keyed by
its package, version and path within the package; a declaration by its name within the enclosing
declaration; anything inside a declaration by its span relative to the declaration's start. Hashes
of data containing origins are therefore the same in every process and survive edits elsewhere in
the file.

== Diagnostics

A diagnostic has an identifier, a severity, a structured message, one primary label, any number of
secondary labels in any file, children (notes, help and causes) with their own labels, fixes, and a
trace for generated code. Messages are sequences of typed parts (text, code, types, names,
numbers) rather than formatted strings. An error that originates outside the compiler, in the file
system, the network or an archive, keeps its chain of causes, which are rendered as `caused by`
children rather than flattened into text.

Each fix has one of four safety levels, following the experience of rustc and Ruff @rustc-diag
@ruff-fixes. _Safe_ fixes preserve meaning and are the only ones applied automatically; _unsafe_
fixes may change behavior; _placeholder_ fixes contain holes to be filled, offered as editor
snippets; _display-only_ fixes are never applied. Cascades are suppressed by poisoning: an
erroneous type or value carries a token obtainable only by emitting an error, and diagnostics
that mention poisoned entities are dropped.

Diagnostic identifiers are binding paths to values of type `lang.diag.Diag`, so they are
namespaced like every other name: the compiler's own are declared in `lang.errors`
(`lang.errors.int.does_not_fit`, shown as `int.does_not_fit`), and a library's lints are declared in
the library. Every identifier has an explanation page, which the `Diag` value embeds. A page has an
example that must produce exactly the diagnostics it is annotated with and a corrected version that
must produce none; both are checked with the test corpus. `vun explain` prints a page offline.

The model is independent of rendering. The terminal renderer is an adapter over the library that
rustc adopted as its only renderer in 2025 @annotate-snippets; others render the ariadne style, a
one-line format, JSON and SARIF. @fig-diagnostic shows the output of the current compiler for the
range error of @sec-integers.

#figure(
  ```term
  $ vun check corpus/reject/integers/does_not_fit.vun
  error[int.does_not_fit]: value does not fit the type `u32`
    ╭▸ corpus/reject/integers/does_not_fit.vun:6:5
    │
  5 │ norm1 := (p: Point) -> u32 {
    │                        ─── expected because of this type
  6 │     p.x.abs() + p.y.abs()   //~ int.does_not_fit
    │     ━━━━━━━━━━━━━━━━━━━━━
    │
    ├ note: the value is `Int(0..=4294967296)`, and `u32` is `Int(0..=4294967295)`
    ╰ help: widen `u32` to hold the whole range, or narrow the value explicitly; …
  ```,
  caption: [A range error as the compiler reports it. The file is part of the rejection corpus,
    and its annotation `//~ int.does_not_fit` states the expected diagnostic.],
) <fig-diagnostic>

Intermediate representations are printed with the origin of every entity, so any line of a dump
can be traced to the source. @fig-dump shows the MIR of the midpoint function of @sec-integers;
`@2:6` is line 2, column 6.

#figure(
  ```term
  $ vun dump mir mid.vun
  fn(_1: a, _2: b) -> _0 { @1:8
    bb0:
      _3 = _1 @2:6
      _4 = _2 @2:10
      _5 = _3 + _4 @2:5
      _6 = _5 / const 2 @2:5
      _0 = _6 @2:5
      return @2:5
  }
  ```,
  caption: [MIR of `mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };` with origins. The sum `_5`
    has type `Int(0..=2^33 - 2)` and needs no check.],
) <fig-dump>

== Lints and conventions

A lint is a compile-time value. It is defined in Vun, runs in the sandboxed evaluator over a
read-only typed view of the program, and is configured by naming its value, so a misspelled lint is
a compile-time error. Pattern lints are written as quotations with holes, never as strings. A
package's lints apply to code that uses its interface, or to any package that enables them.
Levels are `allow`, `warn`, `deny`, `forbid` (which cannot be lowered) and `expect` (which warns
if the lint does not fire), and every override takes a reason. Lints are grouped, with defaults
from `deny` for correctness and naming to `allow` for pedantic and restriction lints.

Naming and organization conventions are lints enabled by default. The case of a binding is
determined by what it is, which compile-time evaluation knows exactly: types, interfaces and
functions returning types are written in `PascalCase`, everything else in `snake_case`, with one
declared list of exemptions for primitive names such as `u8`. Organization lints bound namespace
depth and re-export chains, forbid cycles between directories, and report public signatures that
mention private types. The formatter has no configuration, and its output is stable within an
edition.

== Testing

A test is a binding carrying the attribute `#[test]`, which is plain data. The test runner is a
library program that finds tests by reflection; nothing registers itself at load time. The
parameters of a test select its kind (@tab-tests).

#figure(
  table(
    columns: (auto, auto, 1fr),
    toprule,
    [Parameters], [Kind], [Run by],
    midrule,
    [none], [unit test], [`vun test`],
    [values of ordinary types], [property test], [`vun test` with seeded cases; `vun fuzz`, coverage-guided],
    [`&[u8]`], [fuzz target], [the same],
    [`&mut testing.Bench`], [benchmark], [`vun bench`],
    [`&mut testing.Sim`], [deterministic simulation], [`vun test`; `vun sim --seeds N`],
    bottomrule,
  ),
  caption: [Test kinds, selected by the parameters of the test.],
) <tab-tests>

Inputs of property tests are generated from their types by reflection, reading from a typed choice
stream, so range types bound generation without annotation and shrinking works on the stream for
custom generators too. Because time, network, storage and randomness reach code only through
capabilities, the simulation handle can supply deterministic, seeded substitutes for all of them:
a virtual clock, a seeded scheduler, and a network and disk that drop, reorder, delay and corrupt
on command. This is the approach of deterministic simulation testing @tigerbeetle-fuzz, obtained
without run-time support. Tests run by default in the reference interpreter, which detects every
detectable item of the list of undefined behaviors (@app-ub), so each test run is also a check of
`unsafe` code. They can also run natively, in an emulator or on a board, with the same result
stream.

The compiler's own acceptance tests follow the same principle. The corpus consists of programs
that must be accepted and programs that must be rejected; the latter carry annotations such as
`//~ place.ambiguous_join` on the line where the diagnostic is expected, and the harness compares
the exact set of line and identifier pairs, so a cascade of diagnostics fails the test.

== Packages

The manifest is the build file, `build.vun`, written in Vun. Its package description must be a
literal, so tools and indexes read it without evaluation; its build function is sandboxed
compile-time code that records a plan of steps through a `&mut Build` capability. Everything
configurable in a project, from dependencies and targets to memory maps, lints and flashing
methods, is configured there, with no second configuration format.

- *Identity.* A package is identified by a stable identifier and a major version, and a version by
  the content hash of its source tree. URLs serve only as mirrors.
- *Tree hash.* The hash is a BLAKE3 Merkle tree over directory entries named by path component, so
  it is the same on every platform, a mismatch names the file that differs, and each file's hash is
  the key the compiler's caches already use. BLAKE3 was measured at 2 to 3 GiB/s on one core and
  12 GiB/s on fourteen, against 1.4 GiB/s for hardware-accelerated SHA-256, and it is the only hash
  the toolchain uses.
- *Store.* Fetched packages are kept in one store per user, under paths derived from the hash
  alone. Archives are unpacked in a staging area and verified before they enter the store.
  Lockfiles register their projects as roots, and collection keeps exactly the entries some
  registered lockfile records.
- *Archives and lockfile.* There is no bespoke format. Packages are published as `.tar.zst` with
  fixed metadata, so identical trees give identical archives. The lockfile, `lock.vun`, is a Vun
  literal with one sorted entry per line and no whole-file hash, so merges do not conflict.
- *Resolution.* Resolution is minimal version selection @go-mvs: declared versions are minimums,
  the largest minimum is selected, and different major versions are different packages. There are
  no version ranges and no solver. The root manifest may replace an identity anywhere in the graph
  with a path or a hash; only the root's replacements apply.
- *Parameters, not features.* A dependency is instantiated with compile-time parameters rather
  than globally unified feature flags, so no dependent can change another's build; two
  instantiations that each claim the same peripherals are a visible type error.

@tab-ecosystem summarizes what the package system adopts from existing ecosystems and what it
avoids.

#figure(
  table(
    columns: (1fr, auto, 1.1fr),
    toprule,
    [Adopted or avoided], [Seen in], [Reason],
    midrule,
    [content hash as identity, URLs as mirrors], [Zig, Nix, Roc], [integrity without a central authority],
    [minimal version selection], [Go], [deterministic and explainable, no solver],
    [sorted, line-per-entry integrity file], [Go], [merges without conflicts],
    [deferred logging], [defmt], [a log costs an index and packed bytes on the device],
    midrule,
    [_avoided:_ graph-wide additive features], [Cargo], [one dependent changes another's build],
    [_avoided:_ arbitrary code at build time], [`build.rs`, `build.zig`], [unsandboxed and uncacheable],
    [_avoided:_ file names selecting code], [Go, Hare], [selection invisible in the source],
    [_avoided:_ one URL per dependency], [Zig], [a dead host breaks the build],
    [_avoided:_ a bespoke data format], [ZON], [every tool needs a new parser],
    bottomrule,
  ),
  caption: [Practices adopted from and avoided in existing package ecosystems @unboxed-zig.],
) <tab-ecosystem>

For embedded work, flashing and debugging tools are linked into the driver as libraries, and
deferred logging is library code: a log call parses its format string at compile time, places the
string in a section that is not loaded, and transmits only its index and the packed arguments.
C code is compiled by declared build steps whose outputs are cached by the hash of the tool, its
arguments and its inputs; Vun code can in turn be built as a static library with a C header for
inclusion in a vendor build system.
