#import "../template.typ": *

= Range integers <sec-integers>

Integer overflow is treated by systems languages in one of three ways: as undefined behavior (C), as
a check whose presence depends on the build profile (Rust), or as an unconditional trap (Swift). All
three accept that `a + b` on two machine integers may fail to denote the mathematical sum. Overt
does not: in Overt, arithmetic is exact, and the only operation that can fail to represent a value is
an explicit narrowing.

== Types

Every integer type is a closed interval of the integers, written `Int(lo..=hi)` or, half-open,
`Int(lo..hi)`. We write $"Int"[l, h]$ for the type with $l <= h$. The familiar names are ordinary
bindings in the prelude, for instance `u8 := Int(0..=255)`, and arbitrary widths are not a special
case. A literal $n$ has the singleton type $"Int"[n, n]$, so no separate type of compile-time
integers exists.

#rule[subsumption][
  A value of type $"Int"[l, h]$ may be used where $"Int"[l', h']$ is expected if and only if
  $l' <= l$ and $h <= h'$. This is not a conversion: no representable value changes.
]

== Arithmetic

For operands $a : "Int"[l_1, h_1]$ and $b : "Int"[l_2, h_2]$, the result types are given by interval
arithmetic:

$
  a + b &: "Int"[l_1 + l_2, h_1 + h_2] \
  a - b &: "Int"[l_1 - h_2, h_1 - l_2] \
  a times b &: "Int"[min P, max P], quad P = {l_1 l_2, l_1 h_2, h_1 l_2, h_1 h_2}
$

Division and remainder are defined when $0 in.not [l_2, h_2]$; otherwise the operation is a trap
site, discussed below. With truncating division, the extremes of the quotient are attained at the
corners, so $a div b : "Int"[min Q, max Q]$ with $Q = {"trunc"(x / y) | x in {l_1, h_1}, y in {l_2, h_2}}$.
For a positive divisor range the remainder, whose sign follows the dividend, satisfies

$
  a mod b : "Int"[max(min(l_1, 0), 1 - h_2), space min(max(h_1, 0), h_2 - 1)].
$

#rule[exactness][
  Every value produced by `+`, `-` or `*` at run time lies in the static range of its type. The
  operations therefore denote the mathematical integer operations, and no overflow can occur.
]

The rule follows directly from the soundness of interval arithmetic: each operation is monotone in
each argument on the relevant domain, so its extremes over a box are attained at the box's corners.

#rule[width cap][
  An intermediate whose range needs more than 128 bits of storage is a compile-time error.
  Compile-time-only values have no cap.
]

The cap makes the cost of exactness bounded and visible: a chain of multiplications that would
require a 256-bit intermediate must be narrowed explicitly.

== Narrowing

A value leaves a range only through one of four library functions over `lang` primitives:
`x.to(T)`, which traps when $x in.not T$; `x.wrap(T)`, the modular reduction into $T$, defined when
$T$ has $2^n$ elements; `x.sat(T)`, which clamps to the bounds of $T$; and `x.try_to(T)`, which
yields an optional. Compound assignment is sugar over the first: `x += y` means
`x = (x + y).to(TypeOf(x))`, and `+%=` and `+|=` use `wrap` and `sat`.

#rule[trap sites][
  The trap sites of integer code are exactly: `.to` and compound assignments whose operand range is
  not contained in the target, division or remainder by a divisor whose range contains zero, shifts
  whose amount may exceed the width, and indexing whose index range is not contained in the bounds.
]

Typing is deliberately not flow-sensitive: a comparison `i < n` does not change the type of `i`. An
optimizer may use such facts to delete checks, but the type checker never does, so the type of an
expression never depends on the optimizer. Obtaining a narrower type is explicit, for instance
through `Index.try_to(i)`.

== Consequences

The benefits compound in the code systems programmers write most. In a ring buffer of compile-time
capacity $N$, advancing the head needs neither a check nor an annotation beyond the type:

```ov
Index := Int(0..N);
mut head: Index = 0;
head = (head + 1) % N;      // Int[1, N] mod N is Int[0, N - 1] = Index: no check
buf[head] = byte;           // buf: [N]u8 and head: Index: no bounds check
```

The midpoint `(a + b) / 2` of two `u32` values is correct without the usual rewriting, because the
sum has type $"Int"[0, 2^33 - 2]$ and the quotient fits `u32`. Conversely, a function declared
`(p: Point) -> u32` that returns `p.x.abs() + p.y.abs()` for `i32` coordinates is rejected, because
the sum may reach $2^32$: the overflow that C would compile silently is a type error, reported with
the ranges involved and a fix that widens the result type.

Exactness moves cost rather than removing it. A product of two 64-bit values is a 128-bit multiply,
expensive on 32-bit cores, and a counter incremented with `+=` is a trap site on every iteration. The
toolchain reports both: intermediates wider than the target word are listed on request, and the
iteration form `for i in 0..n` yields indices typed $"Int"[0, n - 1]$ that need no check. Range
analysis on the mid-level representation removes most remaining counter checks. Unused values of a
range are available as niches for the layout of optional types (@sec-memory).
