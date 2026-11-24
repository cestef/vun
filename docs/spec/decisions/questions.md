# Questions, with how they were settled

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
   `layout.private_leak` and `vun pkg api diff`. Settled (decision 44): accept.
6. **Function literal parse rule.** A group with a typed parameter, or followed by `->`, is a
   function; a tuple or parenthesized expression never has that shape, but a reader has to learn
   it. Settled (decision 44): accept; the formatter keeps top-level functions at `name := (`.
7. **`e?` and `?T` share a symbol.** Related meanings ("maybe absent"). Settled (decision 44): accept.
8. **Scalable vectors** (SVE, RVV) have no fixed `N`. Settled (decision 44): a `lang.simd.Scalable(T)` type later; decision 480 starts it as `std.simd.Scalable(T)`.
9. **Large implicit copies.** Settled (decision 44): the `perf.large_copy` lint with a byte threshold.
10. **Integer width cap.** Settled (decision 38): 128 bits on every target, so code stays portable.
11. **Choice-stream API size** for generators. Zig's equivalent churned between releases. Settled (decision 44):
    start minimal (bytes, integers in a range, choose, length).
12. **Hash algorithm** for package identity. First leaned SHA-256 (ubiquitous, hardware-accelerated).
    Settled (decision 50): BLAKE3 everywhere, measured 2x faster than SHA-256 with SHA-NI on one
    core and 9x on 14, over a Merkle tree of directory entries, written `blake3:<base32>`. XXH3 was
    considered for caches and rejected: hashing is far below lexing in cost, and a second algorithm
    would not be safe for caches shared between machines.
13. **Vendoring by default** for firmware projects, per gingerBill's argument. Settled (decision 44): offer
    `vun pkg vendor`, do not default.
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
25. **Stable identities for persisted queries.** Query traces hold content hashes and symbols hash
    by text, but `FileId` and `OriginId` were indices into per-process tables. Settled (decision
    59): stable identities, a file by its package, version and path, an origin anchored to its
    declaration (by name) or relative to it.
