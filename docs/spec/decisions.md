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
| 47 | A compile-time block binding is visible in its own value; `$for` binders are compile-time | accepted | 01 |
| 48 | A directory holds a package when its `build.ov` declares `package`; caches are keyed by content hash | accepted | 12 |
| 49 | `targets` is a default dependency, like `lang` and `std` | accepted | 01, 12 |
| 50 | BLAKE3 everywhere; package hashes are Merkle trees of directory entries, written `blake3:<base32>`; the store nests by algorithm and digest prefix | accepted | 12 |
| 51 | Packages are published as `.ovpkg`: a zstd stream of the snapshot then the files; fetches verify before storing | accepted | 12 |
| 52 | Errors keep their source chains; diagnostics render them as `caused by` children | accepted | 10 |
| 53 | `overt pkg add` trusts an archive on first use and writes its entry as a literal into `.deps`, keeping the manifest's layout; `overt pkg pack` writes `<name>-<version>.ovpkg` | accepted | 12 |
| 54 | A snapshot entry records whether its file is executable; the tree hash has a third entry kind for executable files and `.ovpkg` (version 2) restores the bit | accepted (the user's Nix review) | 12 |
| 55 | Minimal version selection runs over the packages present; every dependency resolves to the version selected for its identity; `overt.lock` records every required version (`dep name version id/major hash [via dependent]`, the id in 16 hex digits), the selected one being the largest, written by `overt pkg fetch` and `add`; the edition is 2026 | accepted | 12 |
| 56 | One store per user in the XDG cache directory (`OVERT_STORE` overrides), entries labeled for people; lockfiles are GC roots registered under `<store>/roots/`; `overt cache list\|gc` | accepted (the user's Nix review) | 12 |
| 57 | The query engine memoizes with verifying traces of interface hashes (BLAKE3 of the value's `Hash`), verified once per revision; cycles recover through a per-query rule | accepted | 13 |
| 58 | Symbols come from one process-wide, sharded, append-only pool: equal by pointer, hashed by the BLAKE3 of their text, ordered by text, so hashes and orders built from them are stable across processes and schedules | accepted | 13 |
| 59 | Stable identities: `FileId` hashes by `<package>@<version>/<path>` (a loose file by its path), `OriginId` by its via and anchor (a declaration by name within its declaration, a node by its span relative to its declaration) | accepted | 10, 13 |
| 60 | `overt pkg update` reads versions from an index speaking a Go-proxy-style protocol (`<index>/<id>/<major>/list`, `.../<version>.ovpkg`); an index is a mirror, never an authority: what it serves is verified by hash | accepted | 12 |
| 61 | The query database persists under `.overt/queries/`, keyed by the toolchain's hash, in the encoding of `overt-codec` (LEB128 varints, length-prefixed sequences, no dependencies; each type says how it persists); first written with facet-postcard, replaced by the user's choice to avoid its ~30 crates | accepted | 13 |
| 62 | Explanation pages stay explicit data: each `Diag` embeds its page, as user lint packages will | accepted | 10 |
| 63 | Binary caches are declared in the root manifest (`.caches` with their ed25519 keys); a cached output is used only when signed by a declared key over (step key, output hash), and built locally otherwise | accepted | 12 |
| 64 | `.replace` in the root manifest substitutes a path or a hash for an identity (`id/major`) anywhere in the graph, and the lockfile records it as a `replace` line | accepted | 12 |
| 65 | A partial graph (absent packages) resolves through the lockfile and neither checks nor writes it; fetching walks the declared graph; an absent package is reported once, and names it could provide are not reported (`ErrorGuaranteed` poisons the scope) | accepted (on the agent's recommendation) | 10, 12 |
| 66 | The compiler's primitives are the top-level `= _` declarations of `lang`, generated as one enum per namespace (`primitives::mem::Mem`) under `Primitive`; holes inside interface requirement structs are requirements | accepted (on the agent's recommendation) | 00, 13 |
| 67 | Dependencies are pure Rust wherever a production-grade crate exists or the format is small enough to read ourselves: own tar reader and trace subscriber, `ruzstd` (`.ovpkg` at its fastest level), BLAKE3's `pure` build (measured 20 to 35% slower here, still far above lexing); TLS keeps `rustls` with `ring`, the one native dependency | accepted | 12, 13 |
| 68 | `lang.Fn(params, result)` is the primitive function type, `(A, B) -> R` being `Fn((A, B), R)`; compile-time-only integers (no width cap) are literals, immutable namespace members, `$` parameters and bindings, operands of `$e`, and operations on those alone; a nominal type is shown by the declaration its literal is written in | accepted (on the agent's recommendation) | 02, 04, appendix A |
| 69 | Appendix B is data: `operators` (token, `lang.ops` interface path, function) and `borrows` (word after `&`, `lang.mem` item) in `lang.syntax.desugar`; `Ord` requires `cmp -> Ordering` and provides `lt`, `le`, `gt`, `ge`; `Eq` provides `ne`; `Ordering` is declared in `lang`; `lang.ops` declares `Sub`, `Mul`, `Div`, `Rem`, `Concat`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr` | accepted (on the agent's recommendation) | 02, appendix B |
| 70 | Visibility levels: a name starting with `_` is restricted to a scope, its declaring namespace unless `#[visible(.within(ns))]`, `#[visible(.package)]` or `#[visible(.workspace)]` widens it; `lang.scope` declares `Scope` and its values, and `.name` with a provided type expected looks in the namespace declaring the type; files and directories take their parent's scope; `names.visibility` for a misplaced attribute | accepted (the user chose the levels and the spelling) | 01 |
| 71 | Conditions are data: the table `forms` of `lang.syntax.desugar` names the condition type (`lang.bool`) and the value on which a condition holds (`lang.bool.true`); the other value of the condition type is falsity; an `if` with `else` has the join of its branch types (integer hull, else equal types); the desugaring tables move from `grammar.ov` to `desugar.ov` | accepted (on the agent's recommendation) | 01, appendix B |
| 72 | Compound assignments are data: the table `compound` of `lang.syntax.desugar` gives each `op=`'s operator and narrowing (`to`, `wrap`, `sat`). A compile-time call's step budget covers its own steps and the memoized steps of the calls it makes, so a call's outcome is a function of the call alone; the budget is 2^24 unless `#[comptime.fuel(n)]` on the declaration of the function called sets another (revised from a budget per evaluation root, which a memoized call cannot honor, and from a budget of a call's own steps, which left chains of calls unbounded); `comptime` joins the attribute namespaces of `std.prelude` | accepted (on the agent's recommendation) | 03, 04, appendix B |
| 73 | Compile-time traps are diagnostics: `comptime.trap` and `comptime.exhausted`, at the evaluation root (a top-level value or a type annotation), labeling the trap site with one "while evaluating" frame per call; the checker computes every top-level value | accepted (on the agent's recommendation) | 03 |
| 74 | Typing uses the expected type where the context gives one: annotations, parameters, the result (tails and `return`), fields, assignments, a binary operator's right operand (the left's type), branches; `.name` is a variant, a variant constructor or a declaration of the expected type; tuples and arrays widen item by item; `match` and `if` take the join of their branches | accepted (on the agent's recommendation) | 01, 04 |
| 75 | Type syntax is complete in evaluation: `[N]T` arrays, `[T]` slices (an array value holding one type, mirroring tuples of types), `?T` and `*T` through the table `type_prefixes`; array literals have the join of their items, `[v; n]` repeats, and an integer index gives the item of an array or slice, through a borrow | accepted (on the agent's recommendation) | 02, appendix B |
| 76 | An impl's `T` may be a type constructor (`lang.Int`, or a function returning a type): the impl covers every type it makes, and its value is a function of the constructor's arguments giving each witness (`#[impl(Narrow(Int))]` on `($r: Range) -> type { struct { ... } }`); methods of built-in types come from such impls in `std`, one mechanism for every type. A file sees the impls of its package and of its transitive dependencies. Method lookup is the type's namespace, then its witnesses, then the defaults of their interfaces | accepted (the user chose impls for methods of built-in types; the rest, including the spelling of constructor impls, on the agent's recommendation) | 01, 02 |
| 77 | A function value is its literal and the compile-time call it was evaluated in, whose parameters its body sees (MIR reads them, and literals built from them, as `Rvalue::Comptime`); a declaration of a struct type is evaluated where the type was built (`MemberValue`); a signature naming `$` parameters is typed per call with them bound; the integer narrowings are `std.int.Narrow`, an impl for `lang.Int` over the primitives `lang.int.{to, wrap, sat, fits}`; primitives read the environment, so `fits` gives a `lang.bool` | accepted (on the agent's recommendation) | 03, 04 |
| 78 | `[N]T` and `[T]` are `lang.mem.Array(N, T)` and `lang.mem.Slice(T)` (the table `forms`), primitive constructors that impls target; a slice's length is the method `xs.len()`, the impl `std.seq.Sequence` over `lang.mem.len`, replacing the field form `xs.len`; `lang.target.usize_max` is read from the target the driver sets (a 64-bit one until targets are modeled); a `$` parameter bound in an instantiation is typed by its value | accepted (the user chose the constructors and `len()`; the rest on the agent's recommendation) | 02, 05, appendix B |
| 79 | `a..b`, `a..=b` and `a..` have type `lang.Bounds(T)`, `T` the integers every item of such bounds can take (`Interval::items`); bounds are accepted where a `lang.Range` is expected and where bounds of wider items are; indexing with bounds takes a part, a slice reached as the object is | accepted (the user chose `lang.Bounds(T)`) | 04, appendix B |
| 80 | Loops are typed: `break v` values join into the loop's value, a loop ending by its head adds `()`; `for p in e` finds its items through the table `iteration` (the witness of `lang.ops.Iterate`, its `Iter` coroutine interface, the payload of `yielded` in what `resume` returns); `lang.ops` holds compiler-provided impls of `Iterate` for `Bounds`, `mem.Slice` and `mem.Array`; iterating a slice through a borrow yields borrows of its items (two corpus programs now read `b.*`); `.*` reads through a borrow or a pointer | accepted (on the agent's recommendation) | 01, appendix B |
| 81 | `&e` is typed through the table `borrows` (a borrow of a type is a type), `unsafe e` and `#[attrs] e` have the type of `e` (attributes on an expression are data, read where they act), struct patterns bind fields by name or position, and each branch of an `if` or `match` is checked against the expected type where it is written; `corpus/accept/syntax/patterns.ov` had two genuine overflows (`u32` squared times 3 in `u64`, a signed area), now `circle(u16)` and `.to(u64)` | accepted (on the agent's recommendation) | 01, 04 |
| 82 | Compile-time evaluation reads types: a declaration's value is typed first (`TypedDecl`, per instance for members of built types) and evaluated with those types, so `.{ }`, `.name` and `.variant(x)` have values; expected types also flow down while evaluating (`Env::eval_as`: fields, items, payloads, field defaults); strings are byte arrays and a shared borrow of a compile-time value is the value; a borrow of `[N]T` is accepted where a borrow of `[T]` is expected (spec 02); MIR builds aggregates and reads fields and items (`Rvalue::{Aggregate, Field, Index}`) | accepted (on the agent's recommendation) | 02, 03, 13 |
| 83 | `$for` and `$if` in type bodies are expanded when the members are read: each step binds the `$for` binder (`Staging`), and what it generates keeps its bindings (members, and function values, whose bodies see them); `$(e)` names are strings known at compile time; a `$` local that is never assigned has the value of its declaration in the call evaluated in (`$Regs := struct { ... }` names its own struct) | accepted (on the agent's recommendation) | 03 |
| 84 | Reflection descriptors are ordinary `lang.meta` structs named by each primitive's declared result type (`fields: ($T: type) -> &[Field]`), filled by field name; `lang.meta.Field` holds `name` and `type` for now (`attrs` and `offset` wait for attribute values and layout); `name_of(T)` is the name diagnostics show; `std.meta` spreads them | accepted (on the agent's recommendation) | 03 |
| 85 | `$for` and `$if` among statements generate statements: a `$for` step is typed and lowered with its binder bound, its types kept per step (`TypedBody::steps`); a `$for` is never a block's value; `$if` as a value (last in a block, or in an expression) has the value of the branch its known conditions select, `else if` included; a step's binder has the type of its item alone | proposed | 03 |
| 86 | Code values are `lang.meta.Code(T)` values holding the expression and what it was written under (the call and every bound binder), so splicing evaluates it where it was written; a quote `#{ e }` has type `Code(T)` for `e: T`; `$c` for `c: Code(T)` has type `T`; an argument for a `Code(T)` parameter is typed against `T` and passed as code (MIR `Rvalue::Quote`); `Code(T)` accepts `Code(U)` when `T` accepts `U`; a code value is passed on by borrow, and `std.meta.Quoted` (an impl for `lang.meta.Code`) gives `c.source()` and `c.span()`, replacing `std.meta.source(c)`/`span(c)`; splicing code that reads run-time locals of another function waits for expansion at the call site | proposed | 03 |
| 87 | Quote levels: each `$` escapes one level, out of the innermost quote or from run time to compile time; a chain of more than `n + 1` inside `n` quotes is `meta.escape`, checked during name resolution; splices evaluate lazily where their code was written, which purity makes equal to evaluating each level as it is generated | proposed | 03 |
| 88 | MIR places carry projections (`.*`, fields, items at an index held in a local), used for every read, write and borrow of a part, so there is one way to reach a part; `.` and indexing dereference once per borrow the typed body shows; `&mode place` is `Rvalue::Ref`; at compile time a shared borrow reads as the value, parts are written by rebuilding the value, and `&mut` waits for a memory model; an index past the end is a compile-time trap | proposed | 13 |
| 89 | Patterns lower to tests in MIR: a test branches to a failure block, binds on success (a copy, or through a borrow a borrow in its mode), and reaches parts through `Position` (tuple items) and `Payload` (a tested variant's payload) projections; `Rvalue::IsVariant` is a condition; `match` tests arms in order and ends in `Unreachable`; `if p = e`, `for p = e` and declarations with a pattern use the same tests; or-patterns that bind wait. An annotation that cannot be evaluated as written is typed first and evaluated with those types, and where `type` is expected a tuple of types (`()` included) is a type | proposed | 01, 13 |
| 90 | A top-level declaration `name: T = _;` outside `lang` has a value: its link symbol (`Value::Linked`), which MIR calls and passes like any function value and which compile-time evaluation cannot run; `[v; n]` evaluates at compile time and is `Rvalue::Repeat` in MIR | proposed | 08, 13 |
| 91 | `e?` goes through the table `early_return` (`lang.ops.Try.branch`, variants `go` and `stop`); `lang.Branch(V, S)` names the result, and `lang.ops` holds the impls for `Option` (`Stop := Option(never)`) and `Result` (`Stop := Result(never, E)`); a type made with an empty enum in place of arguments is accepted where the same constructor is expected (`NominalKind::Empty`), and joins take the accepting type. Closures call through `lang.ops.Call` with `call: (self: &mut Self, args: Params) -> R`: a closure has the type `TyKind::Closure` (its literal, instance, captures and signature), its body is typed with the enclosing body, and its MIR takes the captures before the parameters | accepted (the user chose `never` arguments and `&mut Self`) | 01, 02, appendix B |
| 92 | Coroutines: a function whose body yields returns `TyKind::Frame` (its literal, call, `$` arguments and declared interface), and its body yields what the interface's `resume` carries in `yielded` and ends with what it carries in `done` (the table `iteration` names both); `yield ..e` has what `e` returns. An impl's target may be an interface constructor, covering every type implementing an interface it makes, its value taking that type first (`CoroutineIterate`); the iterators of bounds, slices and arrays are coroutine functions in `lang.ops` over `lang.bounds.{empty, first, ends}`, not holes. A primitive whose declared types name its `$` parameters is typed per call. MIR: `TerminatorKind::Yield`, `Rvalue::Resume`; at compile time a call of a coroutine function gives its frame (`Value::Frame`) and resuming runs it to its next `yield` | accepted (the user chose impls over interface constructors and iterators in `lang`) | 02, 06, appendix B |
| 93 | Layout as spec 05 section 6 states it, computed by the query `Layouts`: integers in their storage with the patterns past their range unused, pointers and borrows one word (a borrow of a slice two) and never null, structs and tuples in order with padding, enums tagged by the smallest integer holding their variants, or in the payload's unused patterns by the niche rule; `#[layout.bits(uN)]`, `#[layout.tag(uN)]`, `#[layout.align(n)]` read as data (`Env::attribute`); `std.meta.size` over `lang.meta.size`; a `_` field takes its space and has no name; `layout.bits_width` when a bit struct's fields do not take its N bits. `#[layout.offset]`, `#[layout.c]` beyond natural order, and frames have no layout yet | proposed | 05 |
| 94 | Place states are checked on MIR (`overt-mir-passes`, query `PlaceStates`): locals start `uninit` and parameters `init`, a join of `init` and `uninit` is ambiguous, parts named statically are tracked on their own, and a borrow parameter's target starts in the state its mode needs and must reach the state it leaves, both columns of the table `borrows` (`place.read_uninit`, `place.ambiguous_join`, `place.out_unwritten`). Moves, kinds and `place.linear_leak` wait for kinds; a stage declares whether it reads MIR, and the corpus treats its missing reports in bodies MIR does not lower as pending | proposed | 05, 13 |
| 95 | Kinds (query `Kinds`): a type is linear when `#[kind.linear]` marks it (on its literal or its declaration), when a part is linear, or when it is a frame; borrows are copy as kinds, their exclusivity being the borrow rules'. Reading a whole linear place moves it; matching or destructuring by value consumes the value, whose parts carry what it held (`Role::Unpacked`); a return with a linear place still held is `place.linear_leak`, a discarded linear result included. A method's receiver is passed as its first parameter asks: a borrow in its mode, or the value | proposed | 05 |
| 96 | `defer s;` is lowered again at every exit of its block (end, `break`, `continue`, `return`, `?`), the latest first, behind a MIR `Defer` terminator naming the linear locals `s` reads whole; the place states decide it statically: it runs where all of them are `init`, is skipped where one is `uninit` (a statement consuming several runs only where all are held; the others are then leaks), and a local `init` on some paths to the exit is `place.ambiguous_join`. An exit inside `s` runs only the statements deferred before it. A borrow whose mode needs and leaves the same state changes no state | proposed | 05, 13 |

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
12. **Hash algorithm** for package identity. First leaned SHA-256 (ubiquitous, hardware-accelerated).
    Settled (decision 50): BLAKE3 everywhere, measured 2x faster than SHA-256 with SHA-NI on one
    core and 9x on 14, over a Merkle tree of directory entries, written `blake3:<base32>`. XXH3 was
    considered for caches and rejected: hashing is far below lexing in cost, and a second algorithm
    would not be safe for caches shared between machines.
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
25. **Stable identities for persisted queries.** Query traces hold content hashes and symbols hash
    by text, but `FileId` and `OriginId` were indices into per-process tables. Settled (decision
    59): stable identities, a file by its package, version and path, an origin anchored to its
    declaration (by name) or relative to it.
