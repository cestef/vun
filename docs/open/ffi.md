# C interop: open

Found while writing `examples/todo` (a GTK 4 program). What was fixed is in `decisions.md`
(673 to 675); what is left:

## Binder (`vun util ffi bind`)

- A pointer is `?*T` unless its header says `_Nonnull` (decision 681): glibc's `__attribute__((nonnull))`
  and GLib's comments say it otherwise, and a call passes `.some(p)`.
- `gtk.h` binds only through a header that includes it (`#include <gtk/gtk.h>`): given as the
  header itself, GTK's include guard (`__GTK_H_INSIDE__`) refuses it.

## Language and std

- `std.os.heap` and `System.env` exist on the C library's runtime only (decision 676): the
  kernel, WASI and Windows runtimes refuse every allocation and set no variable. A heap of their
  own is an allocator to write for each (`mmap` pages, `memory.grow`, `HeapAlloc`).
