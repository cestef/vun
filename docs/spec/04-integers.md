# 04. Numbers

## 1. Integers are ranges

Every integer type is `Int(lo..hi)` (half-open) or `Int(lo..=hi)` (closed). The usual names are
prelude bindings:

```ov
u8    := Int(0..=255);
u3    := Int(0..=7);                        // arbitrary widths are not special
usize := Int(0..=target.usize_max);
```

Types by width come from `lang.int`, so large bounds are computed rather than spelled out:
`unsigned(bits)` is `Int(0..1 << bits)` and `signed(bits)` is
`Int(-(1 << (bits - 1))..1 << (bits - 1))`. `lang.int` declares `u8` to `i128` with them, and
`usize` and `isize` from the target; `std.prelude` re-exports them. Compile-time arithmetic is
exact, so `1 << 64` needs no wider type.

A literal `5` has type `Int(5..=5)`, and a character literal `'a'` has type `Int(97..=97)`. There is
no separate compile-time integer type.

### Rules

1. **Arithmetic is exact.** `a + b` has the range computed by interval arithmetic on the operand
   types. `+`, `-`, `*` never overflow. `(a + b) / 2` is correct for every pair of `u32`.
2. **Widening is implicit.** `Int(R)` is accepted where `Int(S)` is expected iff `R` lies within `S`.
   It cannot lose information, so it is not a conversion.
3. **Narrowing is explicit**, through four `std` methods over `lang` primitives:
   - `x.to(T)`: traps if `x` is out of range;
   - `x.wrap(T)`: modular reduction, for power-of-two ranges;
   - `x.sat(T)`: clamps to the bounds;
   - `x.try_to(T)`: returns `?T`.
4. **Comparisons narrow.** Where a comparison of an immutable binding is known to hold, the
   binding has the narrowed range: in `if i < n { a } else { b }`, `i` is `Int(lo..n.max)` in `a`
   and `Int(n.min..=hi)` in `b`. The same holds for `==` and `!=` at a bound, for conditions joined
   with `and` (in the branch) and `or` (in the other branch), for `for` conditions in the body,
   for match guards in their arm, and after an `if` whose branch cannot fall through
   (`if i >= n { return .none; }`). Mutable places are never narrowed, since any write would undo
   it. Narrowing is part of type checking, defined on the syntax of conditions, and never depends
   on the optimizer.
5. **Width cap.** An intermediate range that needs more than 128 bits is a compile error; narrow an
   intermediate. Compile-time-only values have no cap: literals, immutable namespace members,
   `$` parameters and bindings, operands of `$e`, and operations on those alone.
6. **Division and remainder** by a divisor whose range contains 0 is a trap site; by one whose range
   excludes 0 (`Int(1..)`, a literal, a compile-time value) it is not.
7. **Storage** is the smallest of 8, 16, 32, 64 or 128 bits that holds the range directly, with no
   offset encoding. In a `layout.bits` struct it is the minimal bit count. Unused bit patterns are
   niches (spec 05, section 6).
8. **Bit operations** (`&`, `|`, `^`, `~`, `<<`, `>>`) exist on `uN` types and preserve the type.
   A shift amount whose range exceeds `N-1` is a trap site.
9. **Compound assignment is sugar**: `x += y` is `x = (x + y).to(TypeOf(x))`; `x +%= y` uses `.wrap`
   and `x +|= y` uses `.sat`.

So the trap sites of integer code are exactly: `.to`, a compound assignment, a division by a value
that may be zero, and an index not proven in range. `overt check --traps` lists them.

```ov
N := 64;
Index := Int(0..N);

mut head: Index = 0;
head = (head + 1) % N;    // Int(1..=64) % 64 is Int(0..64), which is Index: no check
buf[head] = byte;         // buf: [N]u8 and head: Index: no bounds check
```

```ov
mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };     // Int(0..=2^33-2) / 2 fits u32: no check
norm1 := (p: Point) -> u64 { p.x.abs() + p.y.abs() }; // Int(0..=2^32) fits u64
```

Declaring `norm1` with `-> u32` is a compile error (2^32 does not fit), which is the overflow bug C
would ship. See spec 10 for the diagnostic.

### Cost model

Exact arithmetic moves cost rather than removing it. Two places need attention, and both are
reported rather than hidden:

- **Wide intermediates.** `u64 * u64` is a 128-bit multiply, expensive on 32-bit cores.
  `overt check --widen` lists every intermediate wider than the target's native word.
- **Loop counters.** `i += 1` is a trap site. `for i in 0..n` yields `i: Int(0..n.max)` with no
  trap, and for compile-time `n` the index type is exact, so indexing an array of that length is
  unchecked. MIR range analysis removes the remaining counter checks when it can prove them.

## 2. Floats

`f16`, `f32`, `f64`: IEEE 754, round to nearest even, no fast-math mode and no flag that changes
float semantics. Operations that trade precision for speed are explicit functions:
`lang.float.mul_add(a, b, c)`, `v.reduce_add(.pairwise(8))` with a specified reduction tree. They
are vectorizable and reproducible at once. There are no implicit int/float conversions.

## 3. Vectors

SIMD is a type, not a mode:

```ov
V := lang.simd.Vector(8, f32);

saxpy := (a: f32, x: &[N]f32, y: &mut [N]f32) {
    $for i in lang.simd.chunks(N, 8) {
        v := V.load(x[i..i + 8]) * V.splat(a) + V.load(y[i..i + 8]);
        v.store(&mut y[i..i + 8]);
    }
};
```

- `Vector(N, T)` implements the `lang.ops` interfaces lane-wise, with the same range rules as
  scalars (lanes of a `u8` vector sum to `Int(0..=510)` lanes).
- Comparisons return `lang.simd.Mask(N)`, an opaque type, never integers.
- Shuffles take compile-time index arrays: `v.shuffle([3, 2, 1, 0])`.
- Backends without vector support lower vectors to scalar loops; the result is identical.
- Instruction-specific intrinsics live in `lang.arch.<arch>`.
- **Target features.** A function marked `#[target.require(.{ .avx2 })]` may use those features. A
  call from a function without them is `unsafe`, or goes through `std.simd.dispatch`, a library
  that selects an implementation at run time from compile-time-generated variants.
- Scalable vectors (SVE, RVV) are an open question (decisions.md).
