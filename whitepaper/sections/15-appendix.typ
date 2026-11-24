#import "../template.typ": *

= Desugaring <app-desugar>

Every form below is a rewrite into `lang` items. The tables named in parentheses are declared in
`lang/syntax/desugar.vun` and read by the compiler as data. Bindings, function literals, `struct`,
`enum`, `match`, `for { }`, `break`, `continue`, `return`, `yield`, `defer`, `unsafe`, `$`, `#{ }`,
`and` and `or` have no desugaring: they are the language.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Syntax], [Meaning],
    midrule,
    [`?T`, `*T`, `..P`], [`lang.Option(T)`, `lang.mem.Ptr(T)`, `lang.Pack(P)` (`type_prefixes`)],
    [`[N]T`, `[T]`], [`lang.mem.Array(N, T)`, `lang.mem.Slice(T)` (`forms`)],
    [`&T`, `&mut T`, `&out T`, `&take T`], [`lang.mem.Ref(T)`, `RefMut(T)`, `RefOut(T)`, `RefTake(T)` (`borrows`)],
    [`&temp T`, `&temp mut T`, `&static T`], [`lang.mem.RefTemp(T)`, `RefTempMut(T)`, `RefStatic(T)` (`borrows`)],
    [`(A, B) -> R`], [`lang.Fn((A, B), R)`, with the markers of the literal],
    [`if c`, `for c`, `a and b`, `!c`], [`c` has type `lang.bool` and holds when it is `lang.bool.true` (`forms`)],
    [`a..b`, `a..=b`], [bounds of type `lang.Bounds(T)`; a `lang.Range` where one is expected (`forms`)],
    [`a + b`, and each operator], [`lang.ops.Add.add(a, b)`, as the table gives (`operators`)],
    [`a < b` in a condition], [narrows the ranges of the immutable integer bindings it compares, where it holds and where it fails (`comparisons`, @sec-integers)],
    [`a == b`, `a < b`], [`lang.ops.Eq.eq(&a, &b)`, `lang.ops.PartialOrd.lt(&a, &b)`],
    [`a[i]`], [`lang.ops.Index.at(&a, i)`, or `at_mut(&mut a, i)` in a place context],
    [`f(a, b)`, `f` not a function literal], [`lang.ops.Call.call(&mut f, (a, b))` (`calls`)],
    [`x op= y`], [`x = (x op y).to(lang.meta.TypeOf(x))` (`compound`)],
    [`x op%= y`, `x op|= y`], [the same with `.wrap(...)` and `.sat(...)` (`compound`)],
    [`e?`], [`match lang.ops.Try.branch(e) { .go(v) => v, .stop(r) => return r }` (`early_return`)],
    [`if p = e { a } else { b }`], [`match e { p => a, _ => b }`],
    [`for c { b }`], [`for { if !c { break; } b }`],
    [`for p = e { b }`], [`for { match e { p => b, _ => break } }`],
    [`for p in e { b }`], [`{ mut it := lang.ops.Iterate.iter(e); defer it.end(); 'l: for { match it.resume() { .yielded(p) => b, .done(r) => break 'l r } } }` (`iteration`)],
    [`yield ..e`], [`for y in e { yield y; }`],
    [`a.f(args)`], [`TypeOf(a).f(<a in the mode of the first parameter of f>, args)`],
    [`[caps] (ps) -> R { b }`], [an anonymous structure of `caps` with an implementation of `lang.ops.Call`],
    [`"text"`], [a `&static [N]u8` in read-only data],
    [`#[a] d`], [`d` with `a` appended to its attribute list],
    [`.name`], [`E.name`, where `E` is the expected type],
    [`[..a, x]`], [`lang.ops.Concat.concat(a, [x])`],
    bottomrule,
  ),
  caption: [The desugaring of Vun into `lang`.],
) <tab-desugar>

= Undefined behavior <app-ub>

Safe code cannot trigger any of the following. That is the definition of safe code. Inside
`unsafe`, the list is exhaustive. The compile-time evaluator detects every item except the fourth
and the fifth.

+ Dereferencing a `*T` that does not point to a live, aligned, initialized `T`.
+ Accessing a place through a raw pointer while an exclusive borrow of an overlapping place is
  live, or writing to memory behind a live `&T` of a cell-free type.
+ Reinterpreting bits as a type for which they are invalid, such as an out-of-range integer or an
  invalid enumeration tag.
+ Calling a foreign function in violation of its contract.
+ Inline assembly that violates its declared operands or clobbers.
+ A data race on non-atomic memory.
+ Moving a fixed value through raw memory operations while it is borrowed.
+ Reaching `lang.panic.unreachable`.
+ Calling a function that requires target features the executing processor lacks.
