# Appendices

## A. `lang` at a glance

`lang` is source code: every item has a signature and a doc comment, and every primitive is a
declaration whose value is `_`.

| Module | Contents |
|--------|----------|
| `lang` | `type`, `Int`, `Fn`, `Float` (`f16`, `f32`, `f64`), `bool`, `never`, `Range`, `Bounds`, `Option`, `Result`, `Step`, `Coroutine`, `Interface`, `interface` |
| `lang.ops` | `Add`, `Sub`, `Mul`, `Div`, `Rem`, `Neg`, `BitAnd`, `BitOr`, `BitXor`, `Not`, `Shl`, `Shr`, `Concat`, `Eq`, `PartialOrd`, `Ord`, `by_cmp`, `Index`, `Call`, `Try`, `Iterate` |
| `lang.mem` | `Ref`, `RefMut`, `RefOut`, `RefTake`, `RefTemp`, `RefTempMut`, `Ptr`, `Slice`, `Array`, `len`, `Cell`, `pointer_to`, `address`, `offset`, `cast`, `read`, `write`, `copy`, `fill`, `volatile_load`, `volatile_store`, `atomic_*`, `prefetch` |
| `lang.meta` | `Code`, `Name`, `quote` primitives, type and declaration info, witnesses, attributes, origins, spans, `embed`, `heap`, `TypeOf` |
| `lang.syntax` | token and node types generated from `grammar.vun` |
| `lang.diag` | `Diag`, `Severity`, `Safety`, reporting primitives |
| `lang.errors` | every compiler diagnostic, as `Diag` values |
| `lang.lint` | `Lint`, `Cx`, `pattern`, `define` |
| `lang.kind` | `linear`, `free`, `fixed`, `owns` |
| `lang.layout` | `bits`, `c`, `tag`, `align`, `offset` |
| `lang.link` | `section`, `export`, `symbol` |
| `lang.abi` | `native`, `c`, `interrupt`, `naked` |
| `lang.panic` | `Trap`, `trap`, `symbols`, `never`, `unreachable` |
| `lang.stack` | `max`, `depth` |
| `lang.opt` | `cold`, `inline`, `level` |
| `lang.hint` | `cold` |
| `lang.flow` | `tail` |
| `lang.float` | `mul_add`, reductions |
| `lang.simd` | `Vector`, `Mask`, `Lane`, `lanes`, `of`, `chunks` |
| `lang.target` | `require`, the `TargetSpec` shape |
| `lang.comptime` | `fuel` |
| `lang.arch` | per architecture (`lang.arch.x86_64`, ...): registers, instruction forms, the `asm { ... }` block, intrinsics |
| `lang.impl` | the `impl` attribute |

## B. Desugaring table

| Syntax | Meaning |
|--------|---------|
| `?T`, `*T`, `..P` | `lang.Option(T)`, `lang.mem.Ptr(T)`, `lang.Pack(P)` (the table `type_prefixes`) |
| `[N]T`, `[T]` | `lang.mem.Array(N, T)`, `lang.mem.Slice(T)` (the table `forms`) |
| `&T`, `&mut T`, `&out T`, `&take T`, `&temp T`, `&temp mut T` | `lang.mem.Ref(T)`, `RefMut(T)`, `RefOut(T)`, `RefTake(T)`, `RefTemp(T)`, `RefTempMut(T)` (the table `borrows`) |
| `if c`, `for c`, `a and b`, `!c` | `c` has the type `lang.bool` and holds when it is `lang.bool.true` (the table `forms`) |
| `(A, B) -> R` | `lang.Fn((A, B), R)` with the attributes of the literal |
| `a..b`, `a..=b` | half-open and closed bounds of type `lang.Bounds(T)`, `T` the integers any such range holds (the table `forms`); where a `lang.Range` is expected, the range they denote |
| `a < b` (each comparison) in a condition | narrows the ranges of immutable integer bindings it compares where it holds and where it fails (the table `comparisons`, spec 04, rule 4) |
| `a + b` (each operator) | `lang.ops.Add.add(a, b)`: the interface and function the table `operators` of `lang.syntax.desugar` gives |
| `a == b`, `a != b` | `lang.ops.Eq.eq(&a, &b)`, `Eq.ne` (provided) |
| `a < b`, `<=`, `>`, `>=` | `lang.ops.PartialOrd.lt(&a, &b)`, `le`, `gt`, `ge` (a total order's from `Ord.cmp` through `lang.ops.by_cmp`) |
| `a[i]` | `lang.ops.Index.at(&a, i)`, or `at_mut(&mut a, i)` in a place context |
| `f(x)` where `f` is not a function | `lang.ops.Call.call(&f, (x,))` |
| `x op= y` | `x = (x op y).to(lang.meta.TypeOf(x))` |
| `x op%= y`, `x op\|= y` | `x = (x op y).wrap(...)`, `.sat(...)` |
| `f(a, b)`, `f` not a function literal | `lang.ops.Call.call(&mut f, (a, b))`, implemented by function values and closures (the table `calls`) |
| `e?` | `match lang.ops.Try.branch(e) { .go(v) => v, .stop(r) => return r }` |
| `f(a, b)`, `f` a closure | `lang.ops.Call(P, R).call(&mut f, (a, b))`: the closure's function with its captures first |
| `if p = e { a } else { b }` | `match e { p => a, _ => b }` |
| `for c { b }` | `for { if !c { break; } b }` |
| `for p = e { b }` | `for { match e { p => b, _ => break } }` |
| `for p in e { b }` (the table `iteration`) | `{ mut it := lang.ops.Iterate.iter(e); defer it.end(); 'l: for { match it.resume() { .yielded(p) => b, .done(r) => break 'l r } } }` |
| `yield ..e` | `for y in e { yield y; }` |
| `a.f(args)` | `TypeOf(a).f(<a in the mode of f's first parameter>, args)` |
| `[caps] (ps) -> R { b }` | an anonymous struct of `caps` with an `impl` of `lang.ops.Call` |
| `"text"` | `&static [N]u8` in read-only data |
| `.name` | `E.name`, where `E` is the expected type |
| `[..a, x]` | `lang.ops.Concat.concat(a, [x])` |
| `#[a] d` | `d` with `a` appended to its attribute list |

Primitives with no desugaring (they are the language): bindings, function literals, `struct`,
`enum`, `match`, `for { }`, `break`, `continue`, `return`, `yield`, `defer`, `unsafe`, `$`, `#{ }`,
`and`, `or`.

## C. Undefined behavior inside `unsafe`

1. Dereferencing a `*T` that does not point to a live, aligned, initialized `T`.
2. Accessing a place through a `*T` while an exclusive borrow of an overlapping place is live, or
   writing to memory behind a live `&T` of a cell-free type.
3. Reinterpreting bits as a type for which they are invalid (an out-of-range `Int`, a bad enum tag).
4. Calling a foreign function in violation of its contract.
5. Inline assembly that violates its declared operands or clobbers.
6. A data race on non-atomic memory.
7. Moving a fixed value through raw memory operations while it is borrowed.
8. Reaching `lang.panic.unreachable`.
9. Calling a function with target features the executing CPU lacks.

The reference interpreter detects every item except 4 and 5. Safe code cannot trigger any of them;
that is the definition of safe.

## D. Grammar sketch

Informal; the normative grammar is `lang/syntax/grammar.vun` plus the parse rules of spec 01,
section 6. Patterns are parsed as expressions (`pattern = expr` below).

```
file        = members
members     = { module_doc | annotated(member) | "$" block_like }
member      = [ "mut" ] expr ( ":=" expr ";" | ":" type [ "=" expr ] ( ";" | "," ) )
statements  = { annotated(statement) } [ expr ]
statement   = member | expr ( assign_op expr )? ";" | block_like [ ";" ] | ";"
annotated(x)= { doc_comment | "#[" items "]" } x
items       = [ item { "," item } [ "," ] ]
item        = annotated(expr [ "=" expr | ":" type ]) | "$" block_like
expr        = prefix { binop prefix }                       (precedence: spec 01, section 6)
prefix      = { "-" | "!" | "~" | "&" [ "temp" ] [ "mut" | "out" | "take" | "static" ] | "*" | "?" | ".."
              | "$" | "unsafe" | "#[" items "]" } postfix
postfix     = primary { "." ( name | int | "$" primary ) | ".*" | "?" | "(" items ")"
              | "[" expr "]" | "{" items "}" }              ("{" only after a name, member or call)
primary     = name | "_" | literal | "." name | ".{" items "}" | group | bracket | block_like
            | "struct" "{" members "}" | "enum" "{" items "}" | "#{" statements "}"
            | ( "return" | "break" [ label ] ) [ expr ] | "continue" [ label ]
            | ( "yield" | "defer" ) expr
group       = "(" items ")" [ "->" type ] [ block ]         (a function when typed, "->" or a body)
bracket     = "[" items [ ";" expr ] "]" [ type | group ]   (array, repeat, array type, closure)
block_like  = { "$" | "unsafe" } ( block | if | for | match | label ":" ( for | block ) )
if          = "if" head block [ "else" ( if | block ) ]
for         = "for" [ head | pattern "in" expr ] block
head        = expr [ "=" expr ]                             (no "{" continues an operand here)
match       = "match" expr "{" { pattern [ "if" expr ] "=>" expr "," } "}"
name        = ident | "\" word
type        = expr
```
