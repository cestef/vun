#import "../template.typ": *

= Diagnostics, testing and packages <sec-tooling>

== Provenance and diagnostics

Every entity in every stage of the compiler holds an origin identifier into a shared table. An origin
records a source span, the way the entity came to exist (written, desugared, instantiated, spliced
from a quotation, named by a computed name, or introduced by a lowering pass) and its parent origin.
Chains are shared, so the cost is one 32-bit index per entity, and an entity without an origin is a
verifier error. The same chains produce debug line tables, `#line` directives in generated C, the
traces attached to diagnostics in generated code, and the source locations reported for run-time
traps.

A diagnostic carries exactly one primary label, any number of secondary labels in any file, notes and
help with their own labels, and fixes. Each fix has one of four safety levels, following the
experience of rustc and Ruff @rustc-diag @ruff-fixes: _safe_ fixes preserve meaning and are the only
ones applied automatically; _unsafe_ fixes may change behavior; _placeholder_ fixes contain holes to be
filled; _display-only_ fixes are never applied. Cascades are suppressed by poisoning: an erroneous
type or value carries a token obtainable only by emitting an error, and diagnostics mentioning poisoned
entities are dropped. Diagnostic identifiers are binding paths to values in `lang` or in libraries, so
they are namespaced like every other name. The model is independent of rendering; the terminal
renderer is an adapter over the library rustc adopted in 2025 @annotate-snippets.

Lints are compile-time values. A lint is defined in Overt, runs in the sandboxed evaluator over a
read-only typed view of the program, and is configured by naming its value, so a misspelled lint is a
compile-time error. Naming and organization conventions are lints enabled by default: the case of a
binding is decided by what it is (types and type-returning functions in `PascalCase`, everything else
in `snake_case`), and layout rules bound nesting depth, re-export chains and cycles between
directories.

== Testing

A test is a binding carrying the attribute `#[test]`, found by the test runner through reflection; no
code registers itself at load time. The parameters of a test select its kind: none for a unit test,
ordinary typed values for a property test whose inputs are generated from the types (range types bound
generation without annotation), a byte slice for a fuzz target, a benchmark handle, or a simulation
handle. Because time, network, storage and randomness reach code only through capabilities, the
simulation handle provides deterministic, seeded substitutes for all of them, which is the approach of
deterministic simulation testing @tigerbeetle-fuzz without run-time support. Tests run by default in
the reference interpreter, which detects every detectable item of the undefined-behavior list.

== Packages

The manifest is the build file, written in Overt. Its package description must be a literal, so tools
read it without evaluation; its build function is sandboxed compile-time code. Packages are identified
by a stable identifier and a major version, and versions by the content hash of their sources, with
URLs serving only as mirrors. Resolution is minimal version selection @go-mvs: declared versions are
minimums, the largest minimum is selected, and different major versions are different packages.
Dependencies are instantiated with compile-time parameters rather than globally unified features, so no
dependent can change another's build.
