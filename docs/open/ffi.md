# C interop: open

Found while writing `examples/todo` (a GTK 4 program). What was fixed is in `decisions.md`
(673 to 675); what is left:

## Binder (`vun util ffi bind`)

- C enums are an integer alias plus constants, since C may hold any integer in one; a Vun enum
  with explicit discriminants (`enum(u32) { a = 0 }`, tags that differ from indices through
  layout, lowering and the evaluator) would bind a C enum whose values are known.
- Every pointer is `?*T`: C says nothing of null, and a call passes `.some(p)`.
- A binding is one file, however many declarations it holds; splitting it per declaring
  directory needs the imports between the files.
- `gtk.h` binds only through a header that includes it (`#include <gtk/gtk.h>`): given as the
  header itself, GTK's include guard (`__GTK_H_INSIDE__`) refuses it.

## Linking

- A package cannot list system libraries that no import names (`deps.system`).
- `std.os.library.load` on the default `linux.x86_64.kernel` target is the `unsupported` stub and
  fails at run time, not when building.

## Language and std

- No enum discriminants or enum to integer conversion; `bool` has no `to(i32)`.
- A generic type cannot be aliased (`X := path.Generic` fails the naming lint, `X: type = ...`
  fails to type check); it is imported instead.
- A struct holding only functions is read by the naming lint as a value.
- No heap allocator in `std` (only `Fixed`); the example wraps `malloc`. `System` has no
  environment access; the example calls `getenv`.
- A generic body is reported at the std line when a linear type is used wrongly
  (`Vec.push` of a linear item), with no note of the call site.
- `did you mean X?` can suggest the name already written when the real error is elsewhere.
- `vun new` accepts `gtk-todo` as a package name, which cannot be written as a namespace; its
  template doc comment still says `hello/`.
- A fresh checkout fails `vun-syntax`'s repository scans: `corpus/planned` is not tracked.
