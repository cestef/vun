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
| 27 | Literal data reads tagged values `.name(a, b)`; node kinds declare their children as tagged slots, one kind per shape, checked by the CST verifier | accepted | 13 |

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
14. **Node fields in the grammar.** Resolved by decision 27.
15. **Typed literal after other operands.** `T{ }` is recognized after a name, a member access or a
    call. An alternative with no position rule is `T.{ }`, reusing `.{`. Lean: keep `T{ }` unless
    the rule proves confusing.
16. **Lowercase primitive type names** (`u8`, `bool`, `type`) versus the `PascalCase` rule for
    types. Lean: an exemption list in `naming.case` for `lang` and the prelude.
17. **Nominal identity of type literals.** Memoization makes `Pair(u16)` one type, but the spec
    does not say whether two identical `struct { raw: u32 }` literals in different declarations
    are one type. Compilers need distinct handle types (`NodeId`, `TypeId`) that do not mix. Lean:
    every `struct`/`enum` literal is nominal, identified by its origin plus its compile-time
    arguments; aliases (`u8 := Int(0..=255)`) stay structural because they are not literals.
18. **Exhaustive `match`.** Not stated anywhere. Lean: `match` must cover every value of its
    scrutinee's type (enums by variant, integers by range); a missing case is an error naming it.
19. **Pattern forms.** Patterns are expressions in the syntax, but their meaning is only given by
    example. Needed: or-patterns (`.a | .b`, reusing `|`), range patterns (`'a'..='z'`), rest
    patterns (`.{ .x = a, .. }`, `[first, ..rest]`, reusing prefix `..`), and binding a whole
    value while matching its shape. Lean: define each as the meaning of the existing operator in
    pattern position, no new symbol; for the whole-value binding, `n := .add(l, r)` in an arm.
20. **Matching through borrows.** Matching `e: &Expr` against `.add(l, r)` must say whether `l` is
    `&Expr` or a copy. Lean: one rule, the binding mode is inherited from the scrutinee's borrow
    (`&` gives `&`, `&mut` gives `&mut`), as with `.` on places.
21. **Arena-tied trees.** Recursive enums through `&T` are scoped values, which the rules already
    allow in locals and in other scoped types. Lean: make this the documented way to build trees
    (`std.mem.Arena` returning `&mut T` tied to the arena borrow), keeping `Own` for trees that
    outlive a scope, and add a corpus program that parses into an arena.
22. **Typed handles.** Index-based graphs need `Handle(T)` tied to one arena or list, with checked
    indexing whose trap sites the optimizer can remove. Lean: a `std.collections` library type
    over a nominal struct (17), no language support.
23. **Collected registrations.** Pass managers and lint registries want every item carrying an
    attribute (`#[pass]`), which conflicts with lazy resolution. Lean: collection is explicit,
    `std.meta.collect(package, attr)` over a package's declarations, evaluated only when called.
24. **Text.** Strings are byte arrays. A compiler needs UTF-8 slices, a growable buffer, an
    interner, hash maps and formatting. Lean: all library (`std.text`, `std.collections`,
    `std.fmt`), with a `Str` type that is a UTF-8-checked `&[u8]`; self-host the literal reader as
    the benchmark of that library.
