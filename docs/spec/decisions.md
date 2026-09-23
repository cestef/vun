# Decisions and open questions

## Decision log

| # | Decision | Status | Where |
|---|----------|--------|-------|
| 1 | Integers are ranges | accepted | 04 |
| 2 | Scoped borrows, no lifetime annotations | accepted | 05 |
| 3 | `.` operates on places: receivers passed per signature, arguments marked | accepted | 01 |
| 4 | Async is first-class through one suspension primitive shared with iterators | accepted | 06 |
| 5 | No builtins: primitives are `lang` declarations whose value is `_` | accepted | 00, 01 |
| 6 | Pluggable backends and linkers; the language privileges none | accepted | 13 |
| 7 | `fixed` kind instead of `Pin` | proposed | 05 |
| 8 | Safe interior mutability is always synchronized, so no `Send`/`Sync` | proposed | 07 |
| 9 | `defer` plus place states replaces `errdefer` | proposed | 05 |
| 10 | One loop keyword, `for` | proposed | 01 |
| 11 | 15 keywords: bindings are `:=`, functions are tuple patterns, `$` for compile time, `_` prefix for privacy, `e?`, `yield ..e` | proposed (draft 2) | 01 |
| 12 | Interfaces are functions from a type to a requirement struct; `#[impl]` registers witnesses | proposed (draft 2) | 02 |
| 13 | Programs are their exports; runtimes are compile-time functions; typed link symbols | proposed (draft 2) | 08 |
| 14 | Coroutines take no resume input; async yields wait requests to the executor | proposed (draft 2) | 06 |
| 15 | Metaprogramming in four layers; quotes `#{ }` with `$` as the single stage operator; `Code(T)` parameters are the macro system | proposed (draft 2) | 03 |
| 16 | Initialization in place; assignment evaluate-then-move (Zig's RLS lesson) | proposed (draft 2) | 05 |
| 17 | Aliasing facts are part of the language | proposed (draft 2) | 05, 09 |
| 18 | Every entity in every IR carries an origin chain | proposed (draft 2) | 10, 13 |
| 19 | Diagnostic ids are binding paths; lints are values; conventions are lints | proposed (draft 2) | 10 |
| 20 | Manifest is `build.ov` with a literal `package`; MVS; content-hash identity; parameters not features | proposed (draft 2) | 12 |
| 21 | Tests are `#[test]` bindings; parameters select the kind | proposed (draft 2) | 11 |
| 22 | Package namespace is `<name>.ov` plus `<name>/`, merged | proposed (draft 2) | 01, 12 |
| 23 | CST is a postorder array anchored one token per node; patterns parse as expressions | proposed | 01, 13 |
| 24 | A backslash takes what follows literally: `\name` escapes a keyword, `\"..."` is a raw string, `\\` a raw line | accepted | 01 |
| 25 | `static` is contextual after `&`, like `out` and `take` | proposed | 01, 05 |
| 26 | Command lines are declared as data (`overt-command`), for the CLI and repository tasks | accepted | 13 |

## Open questions

1. **Borrow precision.** "Tied to all borrowed arguments" is conservative. Lean: keep it; add
   `&noescape` for a parameter only if the corpus shows real pain.
2. **Flow-sensitive narrowing.** Letting `if x < N` refine `x` would remove most `.to` calls but make
   types depend on control flow. Lean: no for v1; decide with corpus data.
3. **Single-threaded shared ownership.** Rule 07.2 forces reference counting to be atomic. Lean:
   accept; arenas and handles cover most uses.
4. **`Code(T)` parameters are lazy at the call site.** The laziness is visible in the signature and
   in editor hints, not at the call. Lean: accept, since it is the whole macro system; revisit if the
   corpus shows confusion.
5. **Privacy by `_`.** Everything is public by default, so an API can leak by omission. Mitigated by
   `layout.private_leak` and `overt pkg api diff`. Lean: accept.
6. **Function literal parse rule.** A group with a typed parameter, or followed by `->`, is a
   function; a tuple or parenthesized expression never has that shape, but a reader has to learn
   it. Lean: accept; the formatter keeps top-level functions at `name := (`.
7. **`e?` and `?T` share a symbol.** Related meanings ("maybe absent"). Lean: accept.
8. **Scalable vectors** (SVE, RVV) have no fixed `N`. Lean: a `lang.simd.Scalable(T)` type later.
9. **Large implicit copies.** Lean: the `perf.large_copy` lint with a byte threshold.
10. **Integer width cap.** Lean: 128 bits on every target, so code stays portable.
11. **Choice-stream API size** for generators. Zig's equivalent churned between releases. Lean:
    start minimal (bytes, integers in a range, choose, length).
12. **Hash algorithm** for package identity: SHA-256 now (ubiquitous, hardware-accelerated), with the
    prefix allowing BLAKE3 later.
13. **Vendoring by default** for firmware projects, per gingerBill's argument. Lean: offer
    `overt pkg vendor`, do not default.
14. **Node fields in the grammar.** Spec 13 wants typed accessors generated from node fields;
    `grammar.ov` lists node kinds only. Lean: add named children per kind when HIR lowering needs
    them, and generate accessors with `cargo xtask grammar`.
15. **Typed literal after other operands.** `T{ }` is recognized after a name, a member access or a
    call. An alternative with no position rule is `T.{ }`, reusing `.{`. Lean: keep `T{ }` unless
    the rule proves confusing.
16. **Lowercase primitive type names** (`u8`, `bool`, `type`) versus the `PascalCase` rule for
    types. Lean: an exemption list in `naming.case` for `lang` and the prelude.
