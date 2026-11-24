#import "../template.typ": *
#import "../figures.typ": interval-product

= Range integers <sec-integers>

Systems languages treat integer overflow in one of a few ways (@tab-overflow). All of them accept
that `a + b` on two machine integers may fail to denote the mathematical sum; they differ in what
happens when it does. In Vun arithmetic is exact, and the only operation
that can fail to represent a value is an explicit narrowing.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Language], [Result of an overflowing `a + b`],
    midrule,
    [C], [undefined for signed types; modular for unsigned types],
    [Rust], [a panic in debug builds; modular in release builds unless checks are enabled],
    [Zig], [a detected fault in safe build modes; undefined in unchecked modes],
    [Swift], [a trap, unless checks are disabled with `-Ounchecked`],
    [Vun], [no overflow: the result type is wide enough for every sum],
    bottomrule,
  ),
  caption: [Treatment of integer overflow.],
) <tab-overflow>

== Types

Every integer type is an interval of the integers, written `Int(lo..=hi)` or, half-open,
`Int(lo..hi)`. We write $"Int"[l, h]$ for the type with $l <= h$. The familiar names are ordinary
declarations in `lang.int`, computed from their width (@sec-principles), and arbitrary ranges such
as `Int(0..=100)` or `Int(-3..4)` are not special cases. A literal $n$ has the singleton type
$"Int"[n, n]$, and a character literal the singleton of its Unicode scalar value, so there is no
separate type of compile-time integers.

#rule[subsumption][
  A value of type $"Int"[l, h]$ may be used where $"Int"[l', h']$ is expected if and only if
  $l' <= l$ and $h <= h'$. This is not a conversion: no representable value changes.
]

Tuples and arrays widen item by item, so `(1, 2)` is accepted where `(u8, u8)` is expected. A
range expression `a..b` has type `lang.Bounds(T)`, where `T` is the integer type of every item such
bounds can contain: for `i: u8` and `n: u8`, `i..n` has type `Bounds(Int(0..=254))`, and
`for k in i..n` binds `k` at that type.

A value is stored in the smallest of 8, 16, 32, 64 or 128 bits that holds its range directly,
without offset encoding; in a bit-exact structure (@sec-memory) it takes the minimal number of
bits. The bit patterns outside the range are unused and are available to the layout of enclosing
types.

== Arithmetic

For operands $a : "Int"[l_1, h_1]$ and $b : "Int"[l_2, h_2]$, the result types are given by
interval arithmetic:

$
  a + b &: "Int"[l_1 + l_2, h_1 + h_2] \
  a - b &: "Int"[l_1 - h_2, h_1 - l_2] \
  a times b &: "Int"[min P, max P], quad P = {l_1 l_2, l_1 h_2, h_1 l_2, h_1 h_2}
$

Division and remainder are total when $0 in.not [l_2, h_2]$; otherwise the operation is a trap
site. With truncating division, the extremes of the quotient are attained at the corners, so
$a div b : "Int"[min Q, max Q]$ with $Q = {"trunc"(x \/ y) | x in {l_1, h_1}, y in {l_2, h_2}}$.
For a positive divisor range the remainder, whose sign follows the dividend, satisfies

$
  a mod b : "Int"[max(min(l_1, 0), 1 - h_2), space min(max(h_1, 0), h_2 - 1)].
$

#rule[exactness][
  Every value produced by `+`, `-` or `*` at run time lies in the static range of its type. The
  operations therefore denote the mathematical integer operations, and no overflow can occur.
]

The rule follows from the soundness of interval arithmetic: each operation is monotone in each
argument on the relevant domain, so its extremes over a box of arguments are attained at the
corners of the box (@fig-product).

#figure(
  interval-product,
  caption: [The product $a b$ over $a in [-3, 4]$ and $b in [-2, 5]$. Its extremes, $-15$ and $20$,
    lie at corners of the box, so `a * b` has type $"Int"[-15, 20]$.],
) <fig-product>

#rule[width cap][
  An intermediate whose range needs more than 128 bits of storage is a compile-time error.
  Compile-time-only values (literals, immutable namespace members, `$` parameters and bindings,
  operands of `$e`, and operations on those alone) have no cap.
]

The cap bounds the cost of exactness and makes it visible: a chain of multiplications that would
need a 256-bit intermediate must be narrowed explicitly. It is the same on every target, so code
stays portable. Bit operations (`&`, `|`, `^`, `~`, `<<`, `>>`) are defined only on types of the
form $"Int"[0, 2^N - 1]$ and preserve the type; a shift whose amount may exceed $N - 1$ is a trap
site.

== Narrowing

A value leaves a range only through one of four library methods, which are one implementation of
`std.int.Narrow` for the constructor `Int` over the primitives of `lang.int`:

- `x.to(T)` traps when $x in.not T$;
- `x.wrap(T)` is the reduction into $T$ modulo its size, $min T + ((x - min T) mod |T|)$, for
  every finite $T$;
- `x.sat(T)` clamps to the bounds of $T$;
- `x.try_to(T)` yields an optional.

Compound assignment is defined over them by a table in `lang`: `x += y` means
`x = (x + y).to(TypeOf(x))`, and the forms `+%=` and `+|=`, and the like for `-` and `*`, use `wrap` and `sat`.

#rule[trap sites][
  The trap sites of integer code are exactly: `.to` and compound assignments whose operand range is
  not contained in the target, division or remainder by a divisor whose range contains zero, shifts
  whose amount may exceed the width, and indexing whose index range is not contained in the bounds.
]

Comparisons narrow. Where a comparison of an immutable binding is known to hold, the binding has
the narrowed range: in `if i < n { a } else { b }`, `i` has type $"Int"[l, max(n) - 1]$ in `a` and
$"Int"[min(n), h]$ in `b`. The same holds for `==` and `!=` at a bound, for conditions joined with
`and` in the branch and with `or` in the other branch, for the condition of a `for` in its body,
for a match guard in its arm, and after an `if` whose branch cannot fall through
(`if i >= n { return .none; }`). Narrowing is defined on the syntax of conditions and applies only
to bindings that cannot change, so the type of an expression never depends on the optimizer.
Mutable places are not narrowed, since any write would invalidate the fact.

== Consequences

The benefits show in the code that systems programmers write most often. In a ring buffer of
compile-time capacity $N$, advancing the head needs neither a check nor an annotation beyond the
type:

```vun
Index := Int(0..N);
mut head: Index = 0;
head = (head + 1) % N;      // Int[1, N] mod N is Int[0, N - 1] = Index: no check
buf[head] = byte;           // buf: [N]u8 and head: Index: no bounds check
```

The midpoint `(a + b) / 2` of two `u32` values is correct without the usual rewriting, because the
sum has type $"Int"[0, 2^33 - 2]$ and the quotient fits `u32`. Conversely, a function declared
`(p: Point) -> u32` that returns `p.x.abs() + p.y.abs()` for `i32` coordinates is rejected,
because the sum may reach $2^32$. The overflow that C would compile without warning is a type
error, reported with the ranges involved and a suggestion to widen the result type or narrow
the value (@fig-diagnostic in @sec-tooling).

Exactness moves cost and does not remove it. The toolchain reports where it moves. A product
of two 64-bit values is a 128-bit multiplication, which is expensive on 32-bit cores;
a report of every intermediate wider than the target's word (`vun check --widen`) is specified and not yet implemented. A counter incremented
with `+=` is a trap site on every iteration, whereas the form `for i in 0..n` yields indices of
type $"Int"[0, n - 1]$ that need no check, and range analysis on the mid-level representation
removes most remaining counter checks.

== Floating point and vectors

The floating-point types `f16`, `f32` and `f64`, one constructor indexed by the width, follow IEEE 754 with rounding to nearest even.
There is no fast-math mode and no flag that changes floating-point semantics; operations that trade
precision for speed are explicit functions, such as `lang.float.mul_add` and reductions with a
specified tree (`v.reduce_add(.pairwise(8))`), which are both vectorizable and reproducible. There
are no implicit conversions between integers and floating-point values; explicit ones are total,
and a float a processor lacks is computed by Vun code, bit-identical to hardware. Every operation whose
result is a NaN gives one canonical NaN (sign clear, only the quiet bit set in the fraction), so a
float result has a single encoding on every target and at compile time, where the compiler
computes floats exactly and rounds once.

A vector is a type. `lang.simd.Vector(N, T)` implements the operator interfaces
lane by lane under the same range rules as scalars, so the lanes of a sum of two `u8` vectors have
type $"Int"[0, 510]$: a lane's result type is the one the lane type's own impl gives. Comparisons return an opaque mask type; shuffles take compile-time index
arrays; backends without vector support lower vectors to scalar code with identical results.
A scalable vector is a vector of the target's width: code over it covers a sequence of any length through a partial load, a partial store and the count of lanes the next vector covers, and so assumes no width.
A function marked `#[target.require(...)]` may use additional instruction-set features, and
calling it from a function without them is `unsafe` or goes through a library dispatcher, planned, that
selects among compile-time-generated variants at run time.
