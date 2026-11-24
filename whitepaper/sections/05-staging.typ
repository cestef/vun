#import "../template.typ": *

= Staging and metaprogramming <sec-staging>

The whole language is available at compile time, and code generation is typed, hygienic and
traceable. Code is never built by formatting strings
or by pasting token streams.

== One stage operator

The sigil `$` has one meaning in every position: _evaluate one stage earlier_. On a parameter,
`($T: type)` requires the argument at compile time; on an expression, `$e` forces evaluation at
compile time; on control flow, `$for` and `$if` unroll or select during compilation, and `${ }` is
a block run entirely at compile time. Inside a quotation, described below, the same sigil means
"at generation time". A compile-time value used at run time becomes a constant without further
notation.

```vun
dot := ($N: usize, a: &[N]f32, b: &[N]f32) -> f32 {
    mut acc: f32 = 0;
    $for i in 0..N { acc += a[i] * b[i]; }   // unrolled; i is a constant in each copy
    acc
};
```

Among statements, each step of a `$for` is the body with the binder bound to one item, typed and
lowered on its own, so an expression such as `a.$(f.name)` may have a different type in each
step. A `$if` may also test a pattern, its binders compile-time. The branch of a `$if` that is not selected is parsed and resolved but not type-checked,
which allows target-specific code to name items that exist only on that target.

== Compile-time semantics

Compile-time evaluation runs the whole language: mutation, loops, recursion, allocation and
coroutines. Four properties constrain it.

- *Determinism.* Compile-time code has no clock, no randomness, no environment, no network and no
  processes. Its only input is the files of the current package that its manifest includes, read through `std.meta.embed`,
  and each file read is a recorded dependency. Work that needs input or output is a build step
  (@sec-tooling).
- *Purity.* As a consequence, every compile-time call is a pure function of its arguments. Calls
  are memoized by the content hash of their arguments, and the memo defines type identity:
  `Pair(u16)` is one type wherever it is written.
- *Target semantics.* Compile-time code uses the target's layout, word size and endianness, not
  the host's (guarantee 4).
- *Bounded evaluation.* Every compile-time call has a deterministic step budget, $2^24$ steps
  unless the attribute `comptime.fuel(n)` on the called function sets another. The budget covers
  the call's own steps and those of the calls it makes, so a call's cost, like its value, is a
  function of the call alone, and a memoized call costs the same wherever it is reached.

A trap during evaluation, such as a division by zero, a failed narrowing or a failed assertion, is
a diagnostic at the value being computed, with the trap site labeled and one "while evaluating" frame for each call in
between (@fig-trap).

#figure(
  ```term
  $ vun check trap.vun
  error[comptime.trap]: computing this at compile time failed: division by zero
    ╭▸ trap.vun:2:15
    │
  1 │ half := ($n: u8) -> u8 { 10 / n };
    │                          ────── it fails here
  2 │ Small: type = Int(0..=half(0));
    │               ━━━━━━━━━━━━━━━━
    │
    ├ help: test the divisor before dividing
    ╰ note: while evaluating this call at trap.vun:2:23
  ```,
  caption: [A trap during compile-time evaluation of a type. The fix suggested by the explanation
    page is to give the parameter the type `Int(1..=255)`, which removes the trap site.],
) <fig-trap>
 Every top-level value is computed, as statics are, so a trap in one is always reported,
and reported once, by that value rather than by each value that reads it. An argument that is not
a value of its parameter's type traps whatever the callee: the array type `[3 - 10]u8` passes
$-7$ as the length parameter `$n: Int(0..)` of `lang.mem.Array`, and is reported there.

== Four layers of code generation

Code generation is organized in four layers (@tab-layers), none of which adds a keyword.

#figure(
  table(
    columns: (auto, 1fr, 1.2fr),
    toprule,
    [Layer], [Mechanism], [Typical use],
    midrule,
    [L0 templates], [bodies with `$` parameters, `$for`, `$if`], [generics, unrolling, most generation],
    [L1 computed names], [`$(e)` in any name position], [generated fields, methods, bindings],
    [L2 witnesses from values], [`#[impl(...)]` on any structure-valued expression], [derivation],
    [L3 code values], [quotes `#{ e }` of type `Code(T)`, spliced with `$`], [assertions, embedded languages, control abstractions],
    bottomrule,
  ),
  caption: [The four layers of code generation.],
) <tab-layers>

*Templates.* A function body with compile-time parameters is already a typed template: it is
type-checked, its names resolve lexically, and its compile-time parts are marked. Most generic
code and most code generation need nothing more.

*Computed names.* Any name position may hold `$(e)`, where `e` evaluates at compile time to a name
or a string. Together with `$for` and `$if` in list contexts (structure and enumeration bodies,
argument lists, array literals, match arms), this makes generated declarations read like
hand-written ones. The following generates a register block from a vendor description file parsed
at compile time:

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

*Witnesses from values.* Because an implementation is registered on any expression producing a
structure (@sec-types), deriving is calling a function. The derived code may itself contain
templates, so generated code runs compile-time code of its own when it is instantiated. Since
generation yields ordinary values that are analyzed lazily, this recursion requires no additional
mechanism.

*Code values.* A quotation `#{ e }` is the code of `e` as a compile-time value of type `Code(T)`,
where `T` is the type `e` would have. Within a quotation, `$` escapes to generation time; outside
one, `$c` for a code value `c` splices it. Quotations nest, and each `$` escapes one level, so
inside $n$ quotations a chain of at most $n + 1$ is meaningful and a longer one is an error. A
parameter of type `Code(T)` receives its argument as code rather than as a value. This property of
a signature is the entire macro system:

```vun
assert := ($cond: Code(bool)) {
    if !$cond { std.panic.fail(.assertion(cond.source(), cond.span())); }
};
assert(len <= N);   // the argument arrives as code; the message quotes it with its span
```

Because the parameter is a compile-time parameter, each call site instantiates `assert` with its
own code, and the check compiles to `if !(len <= N) { ... }` at the call site. An argument for
`Code(T)` is typed as a `T`, and `Code(T)` accepts code of any type that `T` accepts.

== Hygiene and reflection

Hygiene holds by construction. A code value records where it was written, including the
bindings of the enclosing call and of the `$for` steps around it, and a name written inside a
quotation resolves there, whatever scope the code is spliced into. A binder introduced inside a
quotation is fresh. A name that the splice site should see must be computed explicitly with
`$(...)`, which makes the capture visible in the source. This avoids the scope-set machinery
required when raw syntax is spliced @sets-of-scopes @lean-notations, in the manner of typed
multi-stage systems @metaocaml @scala-macros. Because code values are pure and keep their
environment, the result of a splice does not depend on the stage at which it is generated.

Reflection is read-only, and attributes are reflected like fields (`std.meta.attr`). `std.meta` turns types and declarations into ordinary structures declared
in `lang.meta` (fields with their names, types, attributes and offsets; declarations; sizes;
origins), and the compiler fills each descriptor through its handles. Types are created only by writing
type literals, never by assembling descriptors. A _freeze rule_ makes reflection sound: the member
set of a type is fixed when the expression that defines it finishes, and reflecting on a type
still being defined is an error that names the cycle. Without it, a generator could observe its
own output, which is the obstacle other languages meet when generated code must add entries to a
global table. Where a program does need to gather registrations, such as a pass manager or a lint
set, collection is explicit: `std.meta.collect(package, attr)` returns the declarations of a
package that carry an attribute, and is evaluated only when called, so resolution remains lazy
elsewhere.

== Library diagnostics

Compile-time code reports errors with the same diagnostic model as the compiler
(@sec-tooling). `std.diag.error` stops evaluation with a diagnostic that carries a location, a
message, optional help and fixes; the location may point inside a string literal, so an embedded
language can report the exact offending substring. A report without a location is attributed to
the call that led into the reporting code from outside it. The formatting library uses this to
reject, at compile time, a format string whose placeholders do not match its arguments. A report
outside any compile-time form refuses only a program that reaches it, at the call that led to it:
an instruction called outside a block of instructions is refused this way.

== Traceability and cost

Every generated declaration, expression and name records its origin: the quotation or template,
and each generator call up to user code. Diagnostics in generated code therefore point at both the
generated construct and its generator, and navigation on a computed name reaches the `$(...)`
that produced it.

The cost of generation is also reported. Measurements elsewhere show that generated code can
dominate compile time (in one Rust project, procedural macros produced 27,000 lines from 16,000
written, accounting for 16% of type-checking time) @proc-macro-size, and that interpreter design
matters by large factors: a loop of $10^8$ iterations takes 189 s in Zig's compile-time
interpreter against 6 s in CPython @zig-4055, and Clang's constant evaluator became 2.5 to 6.6
times faster when it moved from walking the syntax tree to bytecode @clang-bytecode. Vun's
evaluator therefore lowers bodies once to a register bytecode, runs on its own stack, and persists
memoized calls across builds. `--timings` reports the time of every stage and query; `vun check --meta-stats`
reports the instantiations and generated code size of each generic. Evaluation steps per
generator are specified and not yet reported.
