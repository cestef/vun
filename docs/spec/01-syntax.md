# 01. Syntax

## 1. Keywords

```
mut                                    mutability, everywhere it appears
if else match for break continue return   control
yield defer                            suspension, cleanup
and or                                 short-circuit logic
struct enum                            type literals
unsafe                                 the audit boundary
```

**15 reserved words**, plus **5 contextual** ones: `in` (only after `for <pattern>`), and `out`,
`take`, `static` and `temp` (only after `&`). Lua has 22, Go 25, Rust 39, C 44, Zig 49.

A reserved word becomes an ordinary name when escaped with a backslash: `\unsafe` is the name
`unsafe`. The escape is how a declaration mirrors a foreign or domain name that collides with a
keyword (`Safety := enum { safe, \unsafe }`), and it is the same backslash that takes a string
literally (section 5).

Each remaining keyword earns its place: it changes control flow, changes what may be mutated, opens
a type literal, or marks the safety boundary. Nothing that a library, a symbol or an attribute can
express is a keyword.

### Where the rest went

| Other languages | Vun | Spec |
|-----------------|-------|------|
| `const`, `let`, `var`, `static` | `x := e` binds; `mut x := e` binds mutably; at top level both are statics | 1.3 |
| `fn`, `func`, `proc` | a function is a tuple pattern with a body: `(a: u32) -> u32 { a + 1 }` | 1.4 |
| `pub`, `export`, `private`, `internal` | a leading `_` restricts a name to its namespace; `#[visible(...)]` widens it to an enclosing namespace, the package or the workspace | 1.7 |
| `comptime`, `const fn`, `constexpr` | `$` marks what is known at compile time | 02 |
| `interface`, `trait`, `impl` | `interface(...)` is a `lang` function; `#[impl(...)]` is a `lang` attribute | 02 |
| `extern`, `intrinsic` | a declaration whose value is `_` is provided from outside | 1.3 |
| `as` | `x.to(u8)`, `x.wrap(u8)`, `x.sat(u8)`, `x.try_to(u8)` | 04 |
| `is`, `if let`, `while let` | `if p = e { }`, `for p = e { }` | 1.6 |
| `try`, `catch`, `?` as macro | postfix `e?` | 1.6 |
| `async`, `await` | a function containing `yield`; `yield ..e` delegates | 06 |
| `while`, `loop`, `do` | `for cond { }`, `for { }` | 1.6 |
| `import`, `use`, `package` | dependencies are bindings in scope; modules are members of packages | 1.8 |
| `test`, `bench` | `#[test]` on an ordinary binding | 11 |
| `true`, `false`, `null`, `Self`, `self`, `type` | ordinary names bound in `lang`, or parameters you name yourself | 02 |
| `packed`, `align`, `section`, `callconv`, `volatile`, `inline` | attributes and `lang` functions | 02, 05 |
| `errdefer` | `defer` plus place states | 05 |
| `quote`, `macro` | `#{ ... }` quotes code; `$` splices | 03 |

## 2. One meaning per symbol

Type syntax mirrors value syntax. Patterns mirror constructors. Each symbol means one thing.

| Symbol | Meaning, everywhere |
|--------|---------------------|
| `x := e` | bind `x` to `e` |
| `x: T` | "x has type T": bindings, parameters, fields |
| `x = e` | store `e` into the place `x`; in `if`/`for` heads, match pattern `x` against `e` |
| `(a, b)` | tuple: value `(1, 2)`, type `(u8, bool)`, pattern `(x, y)`; `()` is the empty tuple |
| `(a: A) -> R { }` | function: a typed tuple pattern, a result type, a body |
| `(A) -> R` | function type |
| `[ ]` | sequences: type `[N]T`, value `[a, b]`, index `a[i]`, captures `[&mut n] (x: u8) { }` |
| `.{ }` | anonymous struct value `.{ .x = 1 }` |
| `.name` | `name` looked up in the expected type: `.some(x)`, `.output`, `.millis(500)`, `.{ }`; for a type provided from outside (`T: type = _`), in the namespace declaring it |
| `.*` | dereference (postfix) |
| `&` | borrow: `&x` is a value and `&T` its type; likewise `&mut`, `&out`, `&take`, `&temp` |
| `?` | optional: `?T` is the type; postfix `e?` propagates an absent or failed value |
| `..` | infix: range `0..n`; prefix: spread, `[..a, ..b]`, `yield ..e` |
| `$` | one stage earlier: compile time in ordinary code, generation time inside a quote |
| `#[ ]` | attributes: compile-time values attached to the next declaration or type |
| `#{ }` | quote: a piece of code as a compile-time value |
| `'name` | label on a loop or block |
| `_` | discard in patterns; "provided from outside" as a declaration's whole value |
| `->` | "results in": function types and literals |
| `=>` | "then": match arms |
| `\` | take what follows literally: `\unsafe` is a name, `\"C:\dir"` a string without escapes, `\\` a raw line |

A sequence of types is read by what is expected of it. Where a type is expected, `(u8, bool)` is
the tuple type and `[u8]` the slice type, and `&[u8]` a borrow type; where a sequence of types is
expected, as by a field `floats: &[type]`, `&[f32]` borrows the array holding `f32`.

## 3. Bindings

```vun
limit := 64;                   // immutable binding, type inferred
mut count: u32 = 0;            // mutable binding with an explicit type
mut buf: [4096]u8;             // uninitialized: must be written before it is read (spec 05)
(lo, hi) := bounds(xs);        // any irrefutable pattern binds
```

- Every declaration, at every level (file, struct, block), is a binding. There is no separate
  declaration form for functions, types or modules.
- A top-level binding is a static. A top-level `mut` binding is a mutable static, and every access
  to it is `unsafe` (safe shared state goes through spec 07).
- Declarations at file and struct level are order-independent and resolved lazily. Self-reference
  and mutual recursion need no forward declarations.
- Declarations inside a block are sequential. A compile-time binding (`$R := struct { ... }`) is
  also visible in its own value, so that a type built in a function can name itself, and in nested
  functions, like every compile-time binder; so are the binders of `$for`.
- **No shadowing within a file.** A local binding (in a block, a parameter, a pattern) may not
  reuse a name visible where it is declared in its file: an enclosing local or parameter, or a
  member of an enclosing namespace (the file, a `struct`). Members of a namespace only need names
  distinct within it, since they are reached through it (`Uart.new` beside a top-level `new`).
  Parameter names in a function type (`(symbol: &[u8]) -> Attribute`) only label positions: later
  parameter types may use them, and they shadow nothing. Names from the prelude or a dependency
  may be reused, so adding a name to a library never breaks its users. Inside a function literal, the run-time locals around it are not visible (functions do
  not capture), so they are not shadowed either.
- **Provided from outside.** `name: T = _;` declares a binding whose value is not in the source:
  - inside `lang`, the compiler provides it (primitives);
  - with a `link` attribute, the linker provides it (C imports, typed link symbols, spec 08);
  - inside an interface, the conforming `impl` provides it (spec 02).
  An argument of an impl's interface may be `_`, any (spec 02, section 3). Anywhere else, `_` as a
  value is an error.

## 4. Functions

A function literal is a typed tuple pattern, an optional result type and a body. It is a value like
any other, so naming a function is binding it:

```vun
add := (a: u32, b: u32) -> u64 { a + b };
tick := () { counter.bump(); };                       // result type () may be omitted
Pair := ($T: type) -> type { struct { a: T, b: T } }; // $: known at compile time (spec 02)
```

- A function type is written without names: `(u32, u32) -> u64`. Because parameters are a tuple
  pattern, the type of `add` really is "tuple of `(u32, u32)` to `u64`".
- The body is a block. Its value is its last expression without a trailing `;`.
- `return e` leaves the function early. It is the only way to leave a function other than reaching
  the end of its body, trapping, or `yield`.
- Parse rule: a parenthesized group is a function type when one of its parameters is typed
  (`name: T`) or when `->` follows it, and a function literal when a body follows either; `() { }`
  is a function literal too. A parenthesized expression or a tuple never has that shape.
- `unsafe (p: *u32) -> u32 { ... }` marks a function whose callers must be inside `unsafe`. Its body
  is not implicitly unsafe; unsafe operations inside it still need their own `unsafe { }`.

### Packs

The last parameter may be a **pack**, the rest pattern `..args: P` of the parameter tuple: it
gathers the arguments of a call after the others into a tuple of type `P`, which `args` binds. `P`
is written, or is a `$` parameter the call infers as the tuple of those arguments' types (spec 02,
section 1), so each call shape is its own instantiation. A call passes as many arguments to a
written `P` as it has items (`types.arity`); a pack anywhere else is `types.pack_position`.

```vun
sum := (first: u8, ..rest: (u8, u8)) -> u16 { first + rest.0 + rest.1 };
total := ($P: type, ..xs: P) -> u32 {
    mut t: u32 = 0;
    $for i in 0..lang.meta.items(P).len() { t +|= xs.$(i); }
    t
};
six := sum(1, 2, 3);
none := total();
```

In a function type the pack is `..P`, `lang.Pack(P)` (the table `type_prefixes`): the type of
`sum` is `(u8, ..(u8, u8)) -> u16`, `lang.Fn((u8, lang.Pack((u8, u8))), u16)`. The calling
convention decides how a pack is passed: under Vun's it is its tuple; under the C convention its
items are the C function's variadic arguments (spec 07, section 5).

### Closures

A closure lists what it captures, spelled exactly as the values would be passed:

```vun
mut n: u32 = 0;
inc := [&mut n] (by: u8) { n += by; };   // captures a borrow: scoped (spec 05)
dbl := [x] () -> u64 { x * 2 };          // captures a copy: free
```

A closure is sugar for an anonymous struct of its captures plus an `impl` of `lang.ops.Call`
(appendix B), whose `call` borrows the closure mutably: `call: (self: &mut Self, args: Params)
-> R`, so calling one needs a mutable place. Inside the body, a captured name means the captured
place: through a captured borrow, what it points to. The body sees the binders it captures, so it
is typed with the body it is written in. A literal with no capture list captures nothing and is a
plain function value.

### Calls and methods

`f(a, b)` calls `f`. `a.f(b)` is the method form, and has one rule:

> **`.` operates on places; arguments are values.** `a.f(b)` is `T.f(<a>, b)` where `T` is the
> type of `a`, and `<a>` passes the place `a` in whatever mode the first parameter of `f` asks for:
> by value, `&`, `&mut`, `&out`, `&take` or `&temp`.

A field is not a method: when `f` is a field of `T`, `a.f(b)` calls the function value the field
holds with `b` alone, as `(a.f)(b)` would. A function value, `$` parameters included, is a value as
any other, in a field, an array or a result: a table of rows naming functions is looked up and
called at compile time as a table of numbers is.

Arguments inside `( )` are ordinary expressions, so borrowing them is written out: `f(&mut x)`.
The place `a` is computed before the arguments, and borrowed after them, so `v.push(v.len())`
reads `v` before the `&mut v` its receiver takes begins.
Lookup is `T`'s namespace, then `T`'s impls. If `a: &mut T`, lookup continues to `T`, because a
borrow type has no namespace of its own. If `T` is an owner (a struct with a `#[kind.owns]`
pointer field, such as `Own(U)`) lacking the member, lookup continues to `U`: `.` goes where `.*`
goes, so `node.value` reads a field of an `Own(Node)` or a `&Own(Node)`. There is no other
auto-deref and no `Deref` interface; nothing user-defined runs.

## 5. Types and literals

```vun
Point := struct {
    x: i32,                       // fields: `name: T` separated by `,`
    y: i32 = 0,                   // field default
    origin := Point{ .x = 0 };    // declarations: `name := e;`
    norm1 := (p: Point) -> u64 { p.x.abs() + p.y.abs() };
};

Shape := enum { circle(u32), rect(Point, Point), empty };

p := Point{ .x = 1, .y = 2 };     // typed literal
q: Point = .{ .x = 1 };           // anonymous literal, type from context
s: Shape = .circle(3);            // variant constructor, type from context
xs := [1, 2, 3];                  // [3]Int(1..=3), widened on use
ys := [..xs, 4];                  // spread
zeros: [4096]u8 = [0; 4096];      // repetition
t := (1, true);                   // tuple
```

Literals:
- Integers `1_000`, `0xff`, `0o17`, `0b1010`. A literal `n` has type `Int(n..=n)` (spec 04).
- Characters `'a'` are integer literals of their Unicode scalar value.
- Floats `1.5`, `1e-3`, typed by context or `f64` by default.
- Strings `"text\n"` have type `&static [N]u8`. Escapes are `\n \t \r \0 \\ \" \'`, `\xHH` and
  `\u{H..HHHHHH}`. `\xHH` is one byte of a string; in a character literal it is the scalar value
  `HH`.
- A backslash before a string takes it literally: `\"C:\dir\new"` has no escapes and ends at the
  next `"`. A multi-line string is a run of lines each starting with `\\`, also taken literally, so
  it may contain quotes. So there are two string forms: escaped (`"..."`) and literal (`\"..."`,
  `\\` lines).
- There are no block comments. `//` comments, `///` documents the next declaration, `//!`
  documents the enclosing namespace.

## 6. Control flow

```vun
if a < b { a } else { b }

if .some(v) = cache.get(key) { use(v); }          // `=` in a head is a pattern match

match cmd {
    .read(r) => handle_read(r),
    .write(w) if w.len() > 0 => handle_write(w),
    .write(_) => {},
    .reset => reset(),
}

for { poll(); }                                    // forever
for n < limit { n += 1; }                          // while the condition holds
for x in xs { sum += x; }                          // over an iterator (spec 06)
for .some(job) = queue.pop() { run(job); }         // while the pattern matches

'outer: for row in rows {
    for x in row { if x == 0 { break 'outer; } }
}

config := load(path)?;                             // propagate .err / .none to the caller
```

- `if`, `match`, `for` and blocks are expressions. An `if` with `else` has the type both branches
  can take: the hull of two integer ranges, or their common type; without `else`, it is `()`. `break v` gives a `for` its value; a `for` over
  a coroutine that runs to completion has the coroutine's result as its value (spec 06).
- `for p in e` binds `p` to each item of `e`, through the impl of `lang.ops.Iterate` for the
  type of `e` (the table `iteration`, appendix B). Bounds give their integers, an array value its
  items, and a slice reached through a borrow a borrow of each item: `for c in t.children` binds
  `c: &Tree`, and `for b in bytes { sum +%= b.*; }` reads each byte.
- `e?` on `Result(T, E)` returns `.err(x)` from the enclosing function; on `?T` it returns `.none`.
  It never converts error types; `e.map_err(f)?` converts explicitly. Both come from impls of
  `lang.ops.Try` in `lang` (the table `early_return`): `branch` gives `.go(value)` or
  `.stop(r)`, `r` a `Result(never, E)` or an `Option(never)`, which the function's result type
  must accept.
- `and`, `or` short-circuit; `!` negates.
- `if p = e` and `for p = e` with an irrefutable `p` are errors: use a binding.
- `match` is exhaustive: its arms cover every value of the scrutinee's type (enums by variant,
  integers by range), or the match is an error naming what is missing. `_ =>` covers the rest
  explicitly.
- Evaluation order is left to right, always: operands, arguments, literal fields, captures.
- Every statement ends with `;`, except a block-like expression (`if`, `match`, `for`, `{ }`) used
  as a statement.

### Patterns

Patterns mirror constructors and reuse the operators of expressions; in pattern position each
means the one thing below.

| Pattern | Matches |
|---------|---------|
| `x`, `$x`, `_` | anything, binding it to `x` (known at compile time for `$x`), or discarding it |
| `(p, q)`, `.{ .x = p }`, `T{ .x = p }` | a tuple or struct whose parts match |
| `.name`, `.name(p, q)` | an enum variant and its payload |
| `1`, `'a'`, `"text"`, `lo..hi`, `lo..=hi` | an equal value, or one in the range |
| `p \| q` | a value matching either; both sides bind the same names with the same types |
| `..`, `..rest` | the remaining items of a tuple, array or slice, or the other fields of a struct; `..rest` binds them |
| `n := p` | a value matching `p`, bound whole to `n` as well |
| any other expression | a value equal to it: a constant, `Color.red`, `$f(x)` |

A literal or range pattern is the comparison with the value matched (`value == x`, `lo <= x`),
so an integer in it is an operand as in an expression and need not fit the matched type: `0..=300`
matches every `u8`. Any other value in a pattern is a value of the matched type.

Matching through a borrow binds with the borrow's mode: when the scrutinee is `&E` or `&mut E`,
bindings to parts of the `E` are `&` or `&mut` borrows of those parts, the same way `.` operates on
places (section 4). Otherwise bindings copy or move the parts.

Operator precedence, high to low: postfix (`.`, `()`, `[]`, `.*`, `?`), prefix (`-`, `!`, `&`,
`$`), `* / %`, `+ -`, `++`, `<< >>`, `&`, `^`, `|`, `..`, comparisons (non-associative),
`and`, `or`.

### Parse rules

The grammar is the table in `lang/syntax/grammar.vun` (spec 13, section 5) plus these rules, which
settle every place where a token could continue two constructs:

- **Patterns are expressions in the syntax tree.** Patterns mirror constructors, so `(a, b)`,
  `.some(x)` and `.{ .x = p }` parse the same way in both roles; the role comes from the position
  (left of `:=`, in a parameter, in an `if`/`for` head, in a match arm).
- **A `{` in a head opens the body.** In the head of `if`, `for` and `match`, and after `->`, an
  operand never continues into `{`: `if a == b { }` compares with `b`. Parenthesize to use a
  typed literal there: `if p == (Point{ .x = 0 }) { }`.
- **Typed literals** `T{ ... }` follow a name, a member access or a call: `Point{ }`,
  `lang.target.Spec{ }`, `Pair(u8){ }`. Any other `{` after an operand is a block.
- **Array types.** `[e]` directly followed by a token that can start a type (a name, `[`, `(`,
  `*`, `?`, `&`, `$`, `struct`, `enum`, `#[`, `unsafe`) is the array type `[e]T`; so `[N]u8`,
  `[2][3]u8` and `[_]u8`. `[caps]` followed by a function literal is a closure.
- **Block-like statements end at their closing brace.** A statement that starts with `if`, `match`,
  `for`, a block, a label, or `$`/`unsafe` before one of those, ends after it; the next token starts
  a new statement. Such a statement needs no `;`.
- **Ranges and comparisons do not chain**: `a < b < c` and `a..b..c` are errors. `a..` has no upper
  bound.
- **Lists** (arguments, parameters, items, fields, arms, attributes) are comma-separated and
  accept a trailing comma. In `.{ }`, `T{ }` and argument lists an item may be `name = value`.

## 7. Visibility

One rule: **a name starting with `_` is restricted; it is visible only inside its scope**, and in
the namespaces nested within it. Everything else is public: visible wherever its parent is. The
scope is the namespace that declares the name, unless the `lang` attribute `visible` widens it:

| Attribute | Scope |
|-----------|-------|
| none | the declaring namespace |
| `#[visible(.within(ns))]` | the enclosing namespace `ns`, such as `drivers` or the package itself |
| `#[visible(.package)]` | the declaring package |
| `#[visible(.workspace)]` | the packages of the declaring package's workspace (spec 12) |

```vun
_raw := 0x4000_1000;                  // this namespace
#[visible(.within(drivers))]
_regs := Regs.at(_raw);               // anywhere inside drivers
#[visible(.package)]
_parse := (bytes: &[u8]) -> Frame { ... };
#[visible(.workspace)]
_abi := 3;                            // sibling packages of the workspace
```

- The scope of `.within(ns)` must enclose the declaration; otherwise, and on a public name, the
  attribute is `names.visibility`. Widening never makes a name public: public names are exactly
  those without `_`, so a package's public API is its tree minus underscored names.
- The rule applies uniformly to declarations, struct members, files and directories. A file or
  directory has no declaration to carry the attribute, so its scope is its parent namespace; a
  restricted namespace placed at the package root (`_impl/`) is visible in the whole package.

```
drivers/
  uart.vun          drivers.uart: visible outside the package
  _registers.vun    drivers._registers: visible inside drivers/ only
  _dma/            drivers._dma: visible inside drivers/ only
```

- Restriction is visible at every use site (`regs._raw`) without looking up the declaration; how
  far it reaches is in the declaration.
- `Scope` and its values are declared in `lang.scope`; `.package` is looked up there because
  `Scope` is the expected type (section 2), so the compiler reads the attribute as data.

## 8. Modules and packages

- A file is a struct whose members are its top-level declarations.
- A directory is a struct whose members are its files and subdirectories (without extensions).
- A file and a directory with the same name merge into one namespace: `serial.vun` holds the
  declarations and doc comment of `serial`, and `serial/` holds its children. A name defined in both
  is an error.
- A package is a namespace too. By default its root is `<name>.vun` plus `<name>/` in the package
  directory (spec 12).
- The build declares a package's dependencies, and **each dependency is a binding in scope under
  its declared name**; the package itself is one under its own name. `lang`, `std` and `targets`
  are dependencies like any other, present by default and replaceable.
- The names every file sees without qualification come from the prelude: the public names of the
  namespaces listed in the manifest's `.prelude` (spec 12), `.{ "std.prelude" }` by default, and
  none for `.{}`. It is part of the literal manifest so that tools resolve names without evaluating
  the build.
- A name is looked up in the file (its scopes, then the names spread into its enclosing blocks
  and namespaces, innermost first), then among the package and its dependencies, then in the prelude,
  the first listed namespace first.

```vun
uart := mydrivers.serial.uart;    // a module is a value; binding it is importing it
Baud := uart.Baud;                // re-exporting one name is binding it
```

**Spreads.** A member `..path;` of a namespace (a file or a `struct` body) makes every public
member of the namespace at `path` a member here, and `..path.{ a, b };` makes the listed ones
members. It is the prefix `..` of spreading, applied to a namespace, and it re-exports what it
brings: an umbrella namespace is a list of spreads.

```vun
..lang.int;                        // u8 to i128, usize, isize
..std.mem.{ Arena, copy };         // two names
```

- `path` is a namespace path starting at the package or a dependency, so tools resolve spreads
  from the manifest and the files alone, without evaluation.
- A selected name is a member like a declared one. A name spread whole may not also be declared in
  the same namespace or spread there twice (`names.duplicate`), as with a file and a directory.
- A whole spread brings the public names, so it resolves without evaluation. A selection may name a
  restricted name visible where the spread is written, and any other is `names.private`. A spread
  member keeps the declaration's scope, so re-exporting never widens it.
- `path` may name an enum: its variants are members of its namespace, as `Cond.ne` reads one, so a
  spread brings them, each a value of the enum (`..lang.arch.thumb.Cond;` makes `ne` a name).
- A spread written as a statement of a block brings its names into the block, from where it is
  written to the block's end. They are found after every scope around the name (the block's locals
  and parameters, and the locals of the blocks around it), so a spread shadows nothing and adds no
  name a local could not reuse; a block exports nothing, so nothing is re-exported. A block of
  instructions spreads its architecture so (spec 07, section 6).
