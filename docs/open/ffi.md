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
- `std.os.heap` and `System.env` exist on the C library's runtime only (decision 676): the
  kernel, WASI and Windows runtimes refuse every allocation and set no variable. A heap of their
  own is an allocator to write for each (`mmap` pages, `memory.grow`, `HeapAlloc`).
