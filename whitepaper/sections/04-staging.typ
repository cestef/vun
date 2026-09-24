#import "../template.typ": *

= Staging and metaprogramming <sec-staging>

== One stage operator

A type in Overt is a value of type `type`, and values that must be known during compilation are
marked with `$`. The sigil has one meaning in every position: _evaluate one stage earlier_. On a
parameter, `($T: type)` requires the argument at compile time; on an expression, `$e` forces
evaluation at compile time; on control flow, `$for` and `$if` unroll or select during compilation.
Inside a quotation, described below, the same sigil means "at generation time". A compile-time value
used at run time becomes a constant, with no further notation.

```ov
Pair := ($T: type) -> type { struct { a: T, b: T } };
dot := ($N: usize, a: &[N]f32, b: &[N]f32) -> f32 {
    mut acc: f32 = 0;
    $for i in 0..N { acc += a[i] * b[i]; }
    acc
};
```

Compile-time evaluation runs the whole language: mutation, loops, recursion, allocation and
coroutines. It is deterministic and sandboxed, with the files of the current package as its only
input, and it uses the target's layout rather than the host's. Two consequences follow. Every
compile-time call is a pure function of its arguments, so calls are memoized by content hash, and
the memo is what defines type identity: `Pair(u16)` is one type wherever it is written. And the
compile-time evaluator can serve as the reference semantics of the language (@sec-implementation).

== Four layers of code generation

Code generation is organized in four layers, none of which adds a keyword.

*Templates.* A function body with compile-time parameters is already a typed template: it is
type-checked, its names resolve lexically, and its compile-time parts are marked. Most generic code
and most code generation needs nothing more.

*Computed names.* Any name position may hold `$(e)`, where `e` evaluates at compile time to a name
or a string. Together with `$for` and `$if` in list contexts (struct bodies, enum bodies, argument
lists, array literals, match arms), this lets generated declarations read like hand-written ones:

```ov
Peripheral := ($p: svd.Peripheral) -> type {
    Block := struct {
        $for r in p.registers {
            #[layout.offset(r.offset)]
            $(r.name): Reg(svd.bits(r)),
        }
    };
    Block
};
```

*Witnesses from values.* An interface conformance is registered by the attribute `#[impl(I(T))]` on
any expression producing a struct, so deriving an implementation is calling a function. When `T` is
a type constructor, such as `Int`, the impl is a function of the constructor's arguments and covers
every type the constructor makes; the methods of built-in types are such impls, in the library. The derived
code may itself contain templates, and generated code then runs compile-time code of its own when it
is instantiated; because generation yields ordinary values analyzed lazily, this recursion requires
no additional mechanism.

*Code values.* A quotation `#{ e }` is the code of `e` as a compile-time value of type `Code(T)`,
where `T` is the type `e` would have. Within a quotation `$` escapes to generation time; outside,
`$c` for a code value `c` splices it. Quotations nest, and each `$` escapes one level. A parameter
of type `Code(T)` receives its argument as code rather than as a value; this property of a signature
constitutes the entire macro system:

```ov
assert := ($cond: Code(bool)) {
    if !$cond { std.panic.fail(.assertion(cond.source(), cond.span())); }
};
assert(len <= N);
```

== Hygiene and observation

Hygiene is obtained by construction. A name written inside a quotation resolves where the quotation
is written; a binder introduced inside a quotation is fresh; and a name that the splice site should
see must be computed explicitly with `$(...)`, which makes the capture visible in the source. This
avoids the scope-set machinery required when raw syntax is spliced @sets-of-scopes @lean-notations,
in the manner of typed staging systems @metaocaml @scala-macros.

Reflection is read-only: types are created by writing type literals, never by assembling descriptor
values. A *freeze rule* makes it sound: the member set of a type is fixed when the expression that
defines it finishes, and reflecting on a type still being defined is an error that names the cycle.
Without it, a generator could observe its own output, which is the obstacle other languages meet
when generated code must add implementations to a global table.

== Traceability and cost

Every generated declaration, expression and name records its origin: the quotation or template, and
each generator call up to user code. Diagnostics in generated code therefore point at both the
generated construct and its generator, and navigation on a computed name reaches the `$(...)` that
produced it. The cost of generation is reported as well: measurements elsewhere show that generated
code size can dominate compile time @proc-macro-size, and that interpreter design matters by large
factors @zig-4055 @clang-bytecode. Overt's evaluator therefore lowers bodies once to a register
bytecode, interns values only when they escape into types or constants, and persists memoized calls
across builds.
