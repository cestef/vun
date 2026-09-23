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
| 7 | `fixed` kind instead of `Pin` | accepted | 05 |
| 8 | Safe interior mutability is always synchronized, so no `Send`/`Sync` | accepted | 07 |
| 9 | `defer` plus place states replaces `errdefer` | accepted | 05 |
| 10 | One loop keyword, `for` | accepted | 01 |
| 11 | 15 keywords: bindings are `:=`, functions are tuple patterns, `$` for compile time, `_` prefix for privacy, `e?`, `yield ..e` | accepted | 01 |
| 12 | Interfaces are functions from a type to a requirement struct; `#[impl]` registers witnesses | accepted | 02 |
| 13 | Programs are their exports; runtimes are compile-time functions; typed link symbols | accepted | 08 |
| 14 | Coroutines take no resume input; async yields wait requests to the executor | accepted | 06 |
| 15 | Metaprogramming in four layers; quotes `#{ }` with `$` as the single stage operator; `Code(T)` parameters are the macro system | accepted | 03 |
| 16 | Initialization in place; assignment evaluate-then-move (Zig's RLS lesson) | accepted | 05 |
| 17 | Aliasing facts are part of the language | accepted | 05, 09 |
| 18 | Every entity in every IR carries an origin chain | accepted | 10, 13 |
| 19 | Diagnostic ids are binding paths; lints are values; conventions are lints | accepted | 10 |
| 20 | Manifest is `build.ov` with a literal `package`; MVS; content-hash identity; parameters not features | accepted | 12 |
| 21 | Tests are `#[test]` bindings; parameters select the kind | accepted | 11 |
| 22 | Package namespace is `<name>.ov` plus `<name>/`, merged | accepted | 01, 12 |
| 23 | CST is a postorder array anchored one token per node; patterns parse as expressions | accepted | 01, 13 |
| 24 | A backslash takes what follows literally: `\name` escapes a keyword, `\"..."` is a raw string, `\\` a raw line | accepted | 01 |
| 25 | `static` is contextual after `&`, like `out` and `take` | accepted | 01, 05 |
| 26 | Command lines are declared as data (`overt-command`), for the CLI and repository tasks | accepted | 13 |
| 27 | Literal data reads tagged values `.name(a, b)`; node kinds declare their children as tagged slots, one kind per shape, checked by the CST verifier | accepted | 13 |
| 28 | Struct and enum literals are nominal: identity is the literal's origin plus its compile-time arguments; aliases stay structural | accepted | 02, 03 |
| 29 | `match` is exhaustive; `_ =>` opts out explicitly | accepted | 01 |
| 30 | Pattern forms reuse operators: or `\|`, ranges `..`/`..=`, rest `..`, whole binding `n := p` | accepted | 01 |
| 31 | Matching through a borrow binds with the scrutinee's mode | accepted | 01, 05 |
| 32 | No shadowing within a file: a local may not reuse a name visible in its file; prelude and dependency names may be reused | accepted | 01 |
| 33 | The manifest names the prelude (`.prelude`), so tools read it without evaluating the build | accepted | 01, 12 |
| 34 | Lowercase primitive and prelude type names are an exemption list of `naming.case` | accepted | 10 |
| 35 | Integer comparisons narrow the range types of immutable bindings in the region where they hold | accepted | 04 |
| 36 | Typed literals stay `T{ }` with their position rule | accepted | 01 |
| 37 | `&temp` borrows never flow into the result, so the result is not tied to them | accepted | 01, 05 |
| 38 | Integer intermediates are capped at 128 bits on every target | accepted | 04 |
| 39 | Safe shared ownership is always synchronized; single-threaded sharing uses arenas and handles | accepted | 07 |
| 40 | Trees are built in arenas: `std.mem.Arena` hands out `&mut T` tied to the arena borrow | accepted | 05 |
| 41 | Typed handles are a library (`std.collections.Handle(T)`) over nominal structs | accepted | 05 |
| 42 | Registrations are collected explicitly: `std.meta.collect(package, attr)`, evaluated only when called | accepted | 03 |
| 43 | Text is a library: `Str` is a UTF-8-checked `&[u8]`; buffers, interner, maps and formatting in `std` | accepted | 12 |
| 44 | The leans of questions 4 to 9 and 11 to 13 are accepted as stated below | accepted | various |
| 45 | Integer types by width are `lang.int.unsigned(bits)` and `lang.int.signed(bits)`; `lang.int` declares `u8` to `i128`, `usize`, `isize`, which `std.prelude` re-exports | accepted | 04 |
| 46 | Spreads `..path;` and `..path.{ a, b };` re-export the public members of a namespace; the path starts at the package or a dependency | accepted | 01 |
| 47 | A compile-time block binding is visible in its own value; `$for` binders are compile-time | proposed | 01 |
| 48 | A directory holds a package when its `build.ov` declares `package`; caches are keyed by content hash | proposed | 12 |
| 49 | `targets` is a default dependency, like `lang` and `std` | accepted | 01, 12 |

## Questions, with how they were settled

None is open. Each keeps its reasoning; decisions 27 to 44 record the outcomes.

1. **Borrow precision.** "Tied to all borrowed arguments" is conservative. Settled: keep it as the
   default, and add the `&temp` mode for a parameter whose borrow never reaches the result
   (decision 37).
2. **Flow-sensitive narrowing.** Letting `if x < N` refine `x` removes most `.to` calls but makes
   types depend on control flow. Settled: yes, for integer comparisons of immutable bindings
   (decision 35).
3. **Single-threaded shared ownership.** Rule 07.2 forces reference counting to be atomic. Settled
   (decision 39): accept; arenas and handles cover most uses.
4. **`Code(T)` parameters are lazy at the call site.** The laziness is visible in the signature and
   in editor hints, not at the call. Settled (decision 44): accept, since it is the whole macro system; revisit if the
   corpus shows confusion.
5. **Privacy by `_`.** Everything is public by default, so an API can leak by omission. Mitigated by
   `layout.private_leak` and `overt pkg api diff`. Settled (decision 44): accept.
6. **Function literal parse rule.** A group with a typed parameter, or followed by `->`, is a
   function; a tuple or parenthesized expression never has that shape, but a reader has to learn
   it. Settled (decision 44): accept; the formatter keeps top-level functions at `name := (`.
7. **`e?` and `?T` share a symbol.** Related meanings ("maybe absent"). Settled (decision 44): accept.
8. **Scalable vectors** (SVE, RVV) have no fixed `N`. Settled (decision 44): a `lang.simd.Scalable(T)` type later.
9. **Large implicit copies.** Settled (decision 44): the `perf.large_copy` lint with a byte threshold.
10. **Integer width cap.** Settled (decision 38): 128 bits on every target, so code stays portable.
11. **Choice-stream API size** for generators. Zig's equivalent churned between releases. Settled (decision 44):
    start minimal (bytes, integers in a range, choose, length).
12. **Hash algorithm** for package identity: SHA-256 now (ubiquitous, hardware-accelerated), with the
    prefix allowing BLAKE3 later.
13. **Vendoring by default** for firmware projects, per gingerBill's argument. Settled (decision 44): offer
    `overt pkg vendor`, do not default.
14. **Node fields in the grammar.** Resolved by decision 27.
15. **Typed literal after other operands.** `T{ }` is recognized after a name, a member access or a
    call. An alternative with no position rule is `T.{ }`, reusing `.{`. Settled: keep `T{ }` (decision 36).
16. **Lowercase primitive type names** (`u8`, `bool`, `type`) versus the `PascalCase` rule for
    types. Settled: an exemption list in `naming.case` for `lang` and the prelude (decision 34).
17. **Nominal identity of type literals.** Memoization makes `Pair(u16)` one type, but the spec
    does not say whether two identical `struct { raw: u32 }` literals in different declarations
    are one type. Compilers need distinct handle types (`NodeId`, `TypeId`) that do not mix. Settled
    (decision 28): every `struct`/`enum` literal is nominal, identified by its origin plus its compile-time
    arguments; aliases (`u8 := Int(0..=255)`) stay structural because they are not literals.
18. **Exhaustive `match`.** Not stated anywhere. Settled (decision 29): `match` must cover every value of its
    scrutinee's type (enums by variant, integers by range); a missing case is an error naming it.
19. **Pattern forms.** Patterns are expressions in the syntax, but their meaning is only given by
    example. Needed: or-patterns (`.a | .b`, reusing `|`), range patterns (`'a'..='z'`), rest
    patterns (`.{ .x = a, .. }`, `[first, ..rest]`, reusing prefix `..`), and binding a whole
    value while matching its shape. Settled (decision 30): each is the meaning of the existing operator in
    pattern position, no new symbol; the whole-value binding is `n := .add(l, r)` in an arm.
20. **Matching through borrows.** Matching `e: &Expr` against `.add(l, r)` must say whether `l` is
    `&Expr` or a copy. Settled (decision 31): one rule, the binding mode is inherited from the scrutinee's borrow
    (`&` gives `&`, `&mut` gives `&mut`), as with `.` on places.
21. **Arena-tied trees.** Recursive enums through `&T` are scoped values, which the rules already
    allow in locals and in other scoped types. Settled (decision 40): make this the documented way to build trees
    (`std.mem.Arena` returning `&mut T` tied to the arena borrow), keeping `Own` for trees that
    outlive a scope, and add a corpus program that parses into an arena.
22. **Typed handles.** Index-based graphs need `Handle(T)` tied to one arena or list, with checked
    indexing whose trap sites the optimizer can remove. Settled (decision 41): a `std.collections` library type
    over a nominal struct (17), no language support.
23. **Collected registrations.** Pass managers and lint registries want every item carrying an
    attribute (`#[pass]`), which conflicts with lazy resolution. Settled (decision 42): collection is explicit,
    `std.meta.collect(package, attr)` over a package's declarations, evaluated only when called.
24. **Text.** Strings are byte arrays. A compiler needs UTF-8 slices, a growable buffer, an
    interner, hash maps and formatting. Settled (decision 43): all library (`std.text`, `std.collections`,
    `std.fmt`), with a `Str` type that is a UTF-8-checked `&[u8]`; self-host the literal reader as
    the benchmark of that library.
