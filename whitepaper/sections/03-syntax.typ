#import "../template.typ": *

= Surface syntax <sec-syntax>

The syntax is designed under two constraints: a keyword must change control flow, mutability, the
kind of a type literal, or the safety boundary, and a symbol must mean one thing everywhere it
appears.

== Keywords

Overt reserves fifteen words: `mut`, `if`, `else`, `match`, `for`, `break`, `continue`, `return`,
`yield`, `defer`, `and`, `or`, `struct`, `enum` and `unsafe`; and five contextual ones: `in` after
a `for` pattern, and `out`, `take`, `static` and `temp` after `&`. For comparison, Lua reserves 22 words,
Go 25, Rust 39 and C 44. A backslash takes what follows literally, so `\unsafe` is an ordinary
name, `\"C:\dir"` a string without escapes, and a run of lines starting with `\\` a multi-line
string; no reserved word is ever unavailable as a name. The words other languages spend elsewhere
are recovered as follows.

- *Declarations.* `x := e` binds; `mut x := e` binds mutably. There is no `let`, `const`, `var` or
  `static`: a binding at file level is a static.
- *Functions.* A function literal is a typed tuple pattern, an optional result type and a body:
  `(a: u32, b: u32) -> u64 { a + b }`. Naming a function is binding it.
- *Compile time.* The sigil `$` replaces `comptime`, `constexpr` and similar words (@sec-staging).
- *Visibility.* A leading underscore makes a name private to its enclosing namespace; there is no
  `pub`.
- *Interfaces and conformance.* `interface` is a function in `lang`, and conformance is the `lang`
  attribute `#[impl(...)]`.
- *Foreign and primitive declarations.* A declaration whose value is `_` is provided from outside.
- *Error propagation and delegation.* Postfix `e?` propagates failure; `yield ..e` delegates to a
  sub-coroutine, which removes `try` and `await`.
- *Loops.* `for` alone, `for cond`, `for p in e` and `for p = e` replace `loop`, `while`, and
  `while let`.

== One meaning per symbol

Type syntax mirrors value syntax, and patterns mirror constructors. `(1, true)` is a tuple value,
`(u8, bool)` its type and `(x, y)` a tuple pattern; accordingly `(u32, u32) -> u64` is the type of a
function whose parameter tuple has type `(u32, u32)`. `&x` borrows and `&T` is the type of the
borrow. `.name` always means "the member `name` of the expected type", whether it constructs a
variant (`.some(x)`), selects an enumeration value (`.output`), calls a constructor function
(`.ms(500)`) or builds an anonymous struct (`.{ .x = 1 }`). The prefix `..` always spreads, and the
infix `..` always forms a range.

```ov
Point := struct {
    x: i32,
    y: i32 = 0,
    norm1 := (p: Point) -> u64 { p.x.abs() + p.y.abs() };
};

mid := (a: u32, b: u32) -> u32 { (a + b) / 2 };

if .some(v) = cache.get(key) { use(v); }
for .some(job) = queue.pop() { run(job); }
```

== Places and arguments

Method calls follow one rule: the dot operates on places and arguments are values. In `a.f(b)` the
place `a` is passed in whatever mode the first parameter of `f` declares (by value, shared,
exclusive, write-only or consuming), exactly as `a.x` reads a field without copying `a`. Arguments
between parentheses are ordinary expressions, so borrowing them is explicit: `f(&mut x)`. There is
no further automatic dereferencing and no dereference interface; a borrow type has no namespace of
its own, so method lookup through a borrow reaches the borrowed type in one step.

== Namespaces

A file is a struct whose members are its declarations; a directory is a struct whose members are its
files and subdirectories; a file and a directory of the same name merge into one namespace. Each
dependency declared by the build is a binding in scope under its declared name. Importing a module is
therefore binding it, and re-exporting is binding again:

```ov
uart := drivers.serial.uart;
Baud := uart.Baud;
```

Re-exporting many names reuses the prefix `..` that spreads arrays: the namespace member `..path;`
makes every public member of the namespace at `path` a member here, and `..path.{ a, b };` the
listed ones. The path starts at the package or a dependency, so spreads resolve from the manifest
and the files without evaluation, and an umbrella namespace is a list of spreads. A name spread
whole and also declared in the same namespace is an error, as a name defined in both a file and its
directory is.

The single privacy rule applies uniformly to declarations, members, files and directories: a name
beginning with `_` is visible only within the namespace that declares it and those nested inside it.
Privacy is thus visible at every use site without consulting the declaration.
