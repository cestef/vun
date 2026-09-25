# 03. Metaprogramming

Goal: the whole language at compile time, and code generation that is typed, hygienic, traceable
and pleasant to read. No code is ever built by formatting strings or by pasting token streams.

The design has four layers. Each is ordinary Vun; together they add no keyword.

| Layer | Mechanism | Covers |
|-------|-----------|--------|
| L0 templates | function bodies with `$` parameters, `$for`, `$if` | generics, unrolling, most codegen |
| L1 computed names | `$(name)` in any name position | generated fields, methods, bindings |
| L2 witnesses from values | `#[impl(...)]` on any struct-producing expression | derive |
| L3 code values | `#{ ... }` quotes of type `Code(T)`, spliced with `$` | DSLs, macros, custom control, assertions |

## 1. L0: templates are just code

A function body with compile-time parameters is already a typed template: it is type-checked,
its names resolve lexically, and the compile-time parts are marked with `$`.

```vun
dot := ($N: usize, a: &[N]f32, b: &[N]f32) -> f32 {
    mut acc: f32 = 0;
    $for i in 0..N { acc += a[i] * b[i]; }   // unrolled; i is a constant in each copy
    acc
};
```

Calling a function value known at compile time is always a static call, so templates compose
through ordinary higher-order functions.

Among statements, `$for` and `$if` generate statements: each step of a `$for` is the body with
the binder bound to one item, typed and lowered on its own, so `a.$(f.name)` may have a different
type in each step. A `$for` has no value and is never a block's value. `$if` used as a value,
last in a block or in an expression, has the value of the branch it selects (spec 02, section 1).
`$` on any other expression computes it at compile time; `$(if c { a } else { b })` computes the
whole `if`.

## 2. L1: computed names

A name position may hold `$(e)`, where `e` is a compile-time `lang.meta.Name` or string. This is
the only way to produce a name that is not written literally.

```vun
Peripheral := ($p: svd.Peripheral) -> type {
    Block := struct {
        $for r in p.registers {
            #[layout.offset(r.offset)]
            $(r.name): Reg(svd.bits(r)),
        }
        $for r in p.registers {
            $("reset_" ++ r.name) := (self: &mut Block) { self.$(r.name).write(r.reset); };
        }
    };
    Block
};

Gpioa := Peripheral($svd.parse(std.meta.embed("stm32f4.svd")).peripheral("GPIOA"));
```

`$for` and `$if` are allowed in every list context: struct bodies, enum bodies, argument lists,
array literals, match arms. That is what makes generated declarations read like hand-written ones.

## 3. L2: witnesses from values

Spec 02 already makes `#[impl(I(T))]` accept any expression producing a struct. A derive is a
function returning one; it may itself contain templates, which is the "inception" case:

```vun
// serde/derive.vun
derive := ($T: type) -> type { struct {
    serialize := (v: &T, w: &mut Dyn(Encoder)) -> Result((), Encoder.Error) {
        w.begin_struct(std.meta.name_of(T))?;
        $for f in std.meta.fields(T) {
            key := $if .some(r) = std.meta.attr(f, rename) { r.to } else { f.name };
            w.field(key)?;
            Serialize.serialize(&v.$(f.name), w)?;
        }
        w.end_struct()
    };
}};
```

The derived code contains `$for`, so generated code runs compile-time code of its own when it is
instantiated. Nothing special is needed: generation yields ordinary values, which are analyzed
lazily like everything else.

## 4. L3: code values

### Quotes

`#{ e }` is a **quote**: the code `e`, not its value, as a compile-time value of type
`lang.meta.Code(T)` where `T` is the type `e` would have. Inside a quote, `$` means "evaluate now,
at generation time", exactly as it means "evaluate at compile time" outside one.

```vun
square := ($x: Code(f32)) -> Code(f32) { #{ $x * $x } };
```

Splicing a code value into a later stage is `$c`. So `$` is used for all three: forcing evaluation,
escaping inside a quote, and splicing code.

| Stage of the surrounding code | `$e` where `e: Code(T)` | `$e` where `e` is another value |
|-------------------------------|------------------------|---------------------------------|
| run time | insert the code | evaluate at compile time, embed the constant |
| inside a quote | insert the code | evaluate at generation time, embed the constant |

Quotes nest; `$` escapes one level, `$$` two. Multi-stage generation needs no other syntax. Each
`$` escapes one level, out of the innermost quote around it or, outside every quote, from run time
to compile time, so inside `n` quotes a chain of at most `n + 1` is meaningful; a longer one is
`meta.escape`. Because a code value keeps what it was written under and evaluation is pure, a
spliced value does not depend on when its level is generated.

### Code parameters

A parameter of type `Code(T)` receives its argument as code instead of evaluating it. That is a
property of the signature, visible in documentation and in editor inlay hints, and it is the whole
macro system:

```vun
assert := ($cond: Code(bool)) {
    if !$cond { std.panic.fail(.assertion(cond.source(), cond.span())); }
};

assert(len <= N);   // the argument arrives as code; the message quotes it with its span
```

Because the parameter is `$`, each call site instantiates `assert` with its own code, so the check
compiles to exactly `if !(len <= N) { ... }` at the call site.

A code value is written where its code is: its names resolve there, with the bindings of the call
and of the `$for` steps it was written in, and splicing it at compile time evaluates it there. An
argument for `Code(T)` is typed as a `T`, and `Code(T)` accepts code of any value `T` accepts. A
code value is passed on by borrow (`&Code(T)`), since a `Code(T)` parameter would quote the name
instead: `std.meta.Quoted`, an impl for `lang.meta.Code`, gives `c.source()`, the text as written,
and `c.span()`.

### Reading and building code

- `std.meta.syntax(c)` returns the typed syntax tree of a code value. Its node and token types are
  plain enums in `lang.syntax`, generated from the same grammar file that drives the parser
  (spec 13), so they are always exact and complete.
- Code values are built by quoting, never by constructing trees by hand. The tree view is for
  inspection: DSL compilers, linters, `assert` messages.
- Literal DSLs (regular expressions, SQL, register maps, format strings) are strings parsed at
  compile time. `std.meta.span(s, lo, hi)` gives the source span of a substring of a literal, so
  their errors point inside the literal.

```vun
is_ident := re.compile("[a-z_][a-z0-9_]*");   // parsed to a DFA at compile time; tables in rodata
```

## 5. Names and hygiene

Hygiene comes from construction, not from renaming passes.

- A name written inside a quote resolves where the quote is written (lexical scope), whatever
  scope the code is spliced into.
- A binder introduced inside a quote is fresh: it cannot capture or be captured by names at the
  splice site.
- To bind a name that the splice site can see, compute it explicitly: `$(std.meta.name("tmp"))`.
  The capture is visible in the source.
- `std.meta.fresh("tmp")` returns a unique name for generated helpers.

## 6. Reflection

`std.meta` (over `lang.meta`) turns types and declarations into plain data:

```vun
std.meta.fields(Point)        // [.{ .name = "x", .type = i32, .attrs = ..., .offset = 0 }, ...]
std.meta.decls(Point)         // declarations, with values and attributes
std.meta.size(Point)          // 8
std.meta.attrs(f)             // attributes of a field or declaration
std.meta.name_of(Point)       // "Point"
std.meta.origin(Point)        // the origin chain (spec 10)
std.meta.TypeOf(p.x + 1)      // the type of an expression, typed and never run
std.meta.Frame(blink)         // the frame type of a coroutine function (spec 06)
```

The descriptors are ordinary structs declared in `lang.meta` (`lang.meta.Field`), and each
reflection primitive's declared result type names its descriptor: the compiler fills the fields a
descriptor declares, by name, and a descriptor field it has no fact for leaves the call without a
value. `std.meta` re-exports them. A type's name is the one diagnostics show: a `struct` or `enum`
literal by the declaration it is written in, with its compile-time arguments.

`TypeOf` is ordinary `lang` source, `($T: type, $e: Code(T)) -> type { T }`: its parameter quotes
the argument, so any expression, run-time ones included, is typed and never evaluated, and `T` is
inferred from it. `Frame(f)` is the result type of a coroutine function's signature, so it names
the frames of a function without writing a call; for a function that does not suspend it traps.

Reflection is read-only. Types are created by writing type literals (with L0 and L1), never by
assembling descriptor structs.

**Collection.** `std.meta.collect(package, attr)` returns every declaration of `package` that
carries the attribute `attr`, with its value: the way a pass manager, a lint set or a command table
gathers registrations. It is an ordinary call, evaluated only when something calls it, so name
resolution and evaluation stay lazy everywhere else, and what it depends on is explicit.

**Freeze rule.** A type's member set is fixed when the expression defining it finishes evaluating.
Reflecting on a type that is still being defined is an error naming the cycle. This rules out
generators that observe their own output, which is what makes impls and memoization sound.

## 7. Compile-time semantics

- **The whole language.** Every construct available at run time is available at compile time,
  including mutation, loops, recursion, allocation and coroutines.
- **Values of any type.** `lang.meta.Any` holds a compile-time value with its type
  (`lang.meta.any(T, x)`), for data whose type each call infers, such as the records a build plan
  keeps; it exists at compile time only. `lang.meta.append(T, items, item)` gives the items
  followed by one more, in compile-time memory, as a `&static [T]`.
- **Allocation.** Compile-time code allocates from `std.meta.heap()`. Compile-time memory is
  collected by the compiler; data that reaches run time (through a constant or a type) is frozen
  into read-only data, and pointers into it become static addresses.
- **Determinism.** No clock, no randomness, no environment, no network, no processes. The only
  input is files of the current package, read with `std.meta.embed(path)`, and every file read is a
  recorded dependency. Tasks that need I/O are build steps (spec 12).
- **Purity.** As a consequence, every compile-time call is a pure function of its arguments. Calls
  are memoized by content hash, which also defines type identity: `Pair(u16)` is one type.
- **Mutation.** An exclusive borrow (`&mut x`, `&out x`) is a loan of the place, which `.*`
  reads and writes. A call takes what each loan reaches and gives back what its exclusive
  parameters hold when it returns, which is stored through the loan: since an exclusive borrow is
  the only way to its place while it lives, this is the same as sharing the place, and the call
  stays a pure function of its arguments. A borrow a call returns into a parameter is a loan of
  what the caller lent. A shared borrow reads as the value, which cannot change while it lives.
- **Function values.** A function literal evaluated inside a compile-time call sees that call's
  parameters, as the types it builds do: a method of `Pair(u8)` sees `T` bound to `u8`. A
  signature whose types name `$` parameters is typed per call, with those bound to the call's
  arguments: `x.to(u8)` has type `u8`. `a.f(b)` evaluates as `T.f(a, b)` for the type `T` of the
  value `a`.
- **Target semantics.** Compile-time code uses the target's layout, word size and endianness
  (guarantee 4).
- **Traps.** A trap that fires during evaluation (a division by zero, a failed `.to`, spec 04) is
  an error, `comptime.trap`, at the value being computed: the trap site is labeled and each call
  between them is a "while evaluating" frame. Every top-level value is computed, as statics are
  (guarantee 1), so a trap in one is always reported, once: by that value, not by each value
  that reads it. An argument that is not a value of its parameter's type traps, whatever the
  callee: `[3 - 10]u8` passes `-7` as the `$n: Int(0..)` of `lang.mem.Array`.
- **Budget.** Every compile-time call has a deterministic step budget covering its own steps and
  those of the calls it makes, 2^24 unless `#[comptime.fuel(n)]` on the declaration of the
  function called sets another. A call's steps, like its value, are a function of the call alone,
  so a memoized call spends the same wherever it is reached. Exhausting a budget is
  `comptime.exhausted`, with a full "while evaluating" trace. Compile-time calls run on the evaluator's own stack, never the host's.
- **Traceability.** Every generated declaration, expression and name carries an origin chain: the
  quote or template span, then each generator call site up to user code (spec 10). Errors in
  generated code point at both the generated construct and the code that generated it, and
  go-to-definition on a computed name lands on the `$(...)` that produced it.

## 8. Performance of compile-time evaluation

Measured pain elsewhere (research notes, section 1): Zig's compile-time interpreter runs a 100M
iteration loop about 30 times slower than CPython; Clang's constant evaluator gains 2x to 6x by
moving from AST walking to bytecode; Rust proc-macro output size dominated `cargo check` time in
Bevy. So the evaluator is designed for speed from the start (spec 13):

1. MIR bodies are lowered once to a compact register bytecode, cached in the query database.
2. Memory uses the target byte model with provenance; values are interned only when they escape
   into a type or constant, never on each mutation.
3. Calls are memoized on (function, interned arguments), persisted across builds.
4. A JIT behind the `Evaluator` interface is a later option; it must reproduce the target model
   and is differential-tested against the interpreter.
5. `vun check --meta-stats` reports evaluation time and generated code size per generator, so
   the cost of a derive is visible.
