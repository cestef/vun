#import "../template.typ": *
#import "../figures.typ": keyword-counts

= Surface syntax <sec-syntax>

The syntax is designed under two constraints. A word is reserved only if it changes control flow,
changes what may be mutated, opens a type literal or marks the safety boundary; anything a
library, a symbol or an attribute can express is not a keyword. And a symbol means one thing
everywhere it appears.

== Keywords

Vun reserves fifteen words: `mut`; `if`, `else`, `match`, `for`, `break`, `continue`, `return`;
`yield`, `defer`; `and`, `or`; `struct`, `enum`; and `unsafe`. Five further words are keywords
only in one context: `in` after the pattern of a `for`, and `out`, `take`, `static` and `temp`
after `&`. A backslash takes what follows literally, so `\unsafe` is an ordinary name,
`\"C:\dir"` is a string without escapes, and a run of lines starting with `\\` is a multi-line
string. No reserved word is therefore unavailable as a name, which matters when a declaration
mirrors a foreign or domain name (`Safety := enum { safe, \unsafe }`). @fig-keywords compares
the count with other languages.

#figure(
  keyword-counts,
  caption: [Reserved words in comparable languages. Vun reserves 15, plus 5 contextual words.],
) <fig-keywords>

@tab-where lists how the roles of the words other languages reserve are covered.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Elsewhere], [In Vun],
    midrule,
    [`let`, `const`, `var`, `static`], [`x := e` binds, `mut x := e` binds mutably; at file level both are statics],
    [`fn`, `func`, `proc`], [a function is a typed tuple pattern with a body: `(a: u32) -> u32 { a + 1 }`],
    [`pub`, `private`, `export`], [a leading `_` restricts a name; the attribute `visible` widens its scope],
    [`comptime`, `constexpr`, `const fn`], [`$` marks what is known at compile time],
    [`trait`, `interface`, `impl`], [`interface(...)` is a `lang` function; `#[impl(...)]` a `lang` attribute],
    [`extern`, intrinsics], [a declaration whose value is `_` is provided from outside],
    [`as`], [`x.to(u8)`, `x.wrap(u8)`, `x.sat(u8)`, `x.try_to(u8)`],
    [`if let`, `while let`, `is`], [`if p = e { }`, `for p = e { }`],
    [`try`, `catch`], [postfix `e?`],
    [`async`, `await`], [a function containing `yield`; `yield ..e` delegates],
    [`while`, `loop`, `do`], [`for cond { }`, `for { }`],
    [`import`, `use`, `package`], [dependencies are bindings in scope; modules are members of packages],
    [`test`, `bench`], [`#[test]` on an ordinary binding],
    [`true`, `false`, `self`, `Self`], [names bound in `lang`, or parameters the programmer names],
    [`packed`, `align`, `volatile`, `inline`], [attributes and `lang` functions],
    [`errdefer`], [`defer` together with place states],
    [`quote`, `macro`], [`#{ ... }` quotes code; `$` splices it],
    bottomrule,
  ),
  caption: [Where the roles of common reserved words are covered.],
) <tab-where>

== One meaning per symbol

Type syntax mirrors value syntax, and patterns mirror constructors. `(1, true)` is a tuple value,
`(u8, bool)` its type and `(x, y)` a tuple pattern; accordingly `(u32, u32) -> u64` is the type of
a function whose parameter tuple has type `(u32, u32)`. `&x` borrows and `&T` is the type of the
borrow. `.name` always means the member `name` of the expected type, whether it constructs a
variant (`.some(x)`), selects an enumeration value (`.output`), calls a constructor function
(`.ms(500)`) or builds an anonymous structure (`.{ .x = 1 }`). The prefix `..` always spreads and
the infix `..` always forms a range. @tab-symbols lists the symbols whose meaning is fixed.

#figure(
  table(
    columns: (auto, 1fr),
    toprule,
    [Symbol], [Meaning in every position],
    midrule,
    [`x := e`], [bind `x` to `e`],
    [`x: T`], [`x` has type `T`: bindings, parameters, fields],
    [`x = e`], [store `e` into the place `x`; in an `if` or `for` head, match the pattern `x` against `e`],
    [`( )`], [tuple value, tuple type, tuple pattern; `()` is the empty tuple],
    [`[ ]`], [sequences: type `[N]T`, value `[a, b]`, index `a[i]`, closure captures],
    [`.name`, `.{ }`], [member of the expected type; anonymous structure],
    [`.*`], [dereference, postfix],
    [`&`], [borrow: `&x` is a value and `&T` its type; likewise `&mut`, `&out`, `&take`, `&temp`],
    [`?`], [`?T` is an optional type; postfix `e?` propagates an absent or failed value],
    [`..`], [infix: range; prefix: spread (`[..a, x]`, `yield ..e`, `..path;` in a namespace)],
    [`$`], [one stage earlier: compile time in ordinary code, generation time inside a quote],
    [`#[ ]`, `#{ }`], [attributes; quotation of code],
    [`_`], [discard in patterns; "provided from outside" as a declaration's value],
    [`->`, `=>`], ["results in" for functions; "then" for match arms],
    [`\`], [take what follows literally],
    bottomrule,
  ),
  caption: [Symbols with a fixed meaning.],
) <tab-symbols>

```vun
Point := struct {
    x: i32,
    y: i32 = 0,
    norm1 := (p: Point) -> u64 { p.x.abs() + p.y.abs() };
};

mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };

if .some(v) = cache.get(key) { use(v); }
for .some(job) = queue.pop() { run(job); }
```

== Bindings and functions

Every declaration, at every level, is a binding. Declarations at file and structure level are
order-independent and resolved lazily, so self-reference and mutual recursion need no forward
declarations; declarations inside a block are sequential. A top-level binding is a static, and a
top-level `mut` binding is a mutable static whose every access is `unsafe`. A declaration may be
left uninitialized (`mut buf: [4096]u8;`), which costs nothing; reading it before it is written is
a compile-time error (@sec-memory).

A local binding may not reuse a name visible where it is declared in its file: an enclosing local
or parameter, or a member of an enclosing namespace. Names from the prelude or a dependency may be
reused, so that adding a name to a library never breaks its users.

A function literal is a typed tuple pattern, an optional result type and a body, and naming a
function is binding it. A parenthesized group is a function type when one of its parameters is
typed or when `->` follows it, and a function literal when a body follows either. A closure lists
its captures in brackets, spelled as the values would be passed (`[&mut n] (by: u8) { n += by; }`),
and is equivalent to an anonymous structure of its captures with an implementation of
`lang.ops.Call`. A function literal without a capture list captures nothing.

Because the parameters are a tuple pattern, variadic functions need no construct of their own: a
rest pattern `..args: P` as the last parameter gathers the arguments after the others into a tuple
of type `P`, either written or inferred as a `$` parameter, so that each call shape is an
instantiation. The calling convention decides how that tuple is passed: as a tuple under Vun's, and
as C's variadic arguments under `#[abi.c]`, which is how a C import such as `open` is declared.

== Places and arguments <sec-syntax-places>

Method calls follow one rule: the dot operates on places, and arguments are values. In `a.f(b)` the
place `a` is passed in whatever mode the first parameter of `f` declares (by value, shared,
exclusive, write-only, consuming or temporary), in the same way that `a.x` reads a field without
copying `a`. Arguments between parentheses are ordinary expressions, so borrowing them is explicit:
`f(&mut x)`. The receiver place is computed before the arguments and borrowed after them, so
`v.push(v.len())` does not conflict.

Method lookup searches the type's namespace, then the implementations visible to the file, then
the defaults of their interfaces. A borrow type has no namespace of its own, so lookup through a
borrow reaches the borrowed type in one step. An owning structure, one whose pointer field carries
the attribute `kind.owns` such as `Own(T)`, continues lookup to its target when it lacks the
member. These are the only forms of automatic dereferencing, and no user-defined code runs as part
of them.

== Control flow and patterns

`if`, `match`, `for` and blocks are expressions. An `if` with `else` has the join of the types of
its branches, which for integers is the hull of their ranges (@sec-integers). One loop keyword
covers four forms: `for { }` loops forever, `for c { }` while a condition holds, `for p in e { }`
over an iterator (@sec-suspension), and `for p = e { }` while a pattern matches. `break v` gives a
loop its value. `match` is exhaustive: its arms must cover every value of the scrutinee's type,
enumerations by variant and integers by range, and `_ =>` covers the rest explicitly. Evaluation
order is left to right everywhere.

Patterns are parsed as expressions and reuse their operators: `p | q` matches either alternative,
`lo..=hi` matches a range, `..rest` binds the remaining items of a sequence or the remaining fields
of a structure, and `n := p` binds the whole value while matching its shape. Matching through a
borrow binds with the borrow's mode: when the scrutinee is `&E`, bindings to its parts are shared
borrows of those parts.

== Namespaces and visibility

A file is a structure whose members are its declarations; a directory is a structure whose members
are its files and subdirectories; a file and a directory of the same name merge into one
namespace. Each dependency declared by the build is a binding in scope under its declared name.
Importing a module is therefore binding it, and re-exporting is binding again:

```vun
uart := drivers.serial.uart;
Baud := uart.Baud;
..std.mem.{ Arena, copy };        // re-export two names
..lang.int;                       // re-export every public name
```

Re-exporting many names reuses the prefix `..` that spreads arrays. The path of a spread starts at
the package or a dependency, so spreads resolve from the manifest and the files without evaluation,
and an umbrella namespace is a list of spreads. The names every file sees without qualification
come from the prelude, which the manifest names as a literal list of namespaces.

A single rule governs privacy for declarations, members, files and directories: a name beginning
with `_` is restricted to a scope, and visible only within that scope and the namespaces nested in
it. The scope is the declaring namespace unless the `lang` attribute `visible` widens it to an
enclosing namespace, the package or the workspace:

```vun
_raw := 0x4000_1000;                  // this namespace
#[visible(.within(drivers))]
_regs := Regs.at(_raw);               // anywhere inside drivers
#[visible(.package)]
_parse := (bytes: &[u8]) -> Frame { ... };
```

Restriction is visible at every use site without consulting the declaration, and widening never
makes a name public: the public interface of a package is its namespace tree minus the names that
begin with an underscore.
