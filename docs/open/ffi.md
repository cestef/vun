# C interop: open

Found while writing `examples/todo` (a GTK 4 program). What was fixed is in `decisions.md`
(673 to 675); what is left:

## Binder (`vun util ffi bind`)

- A pointer is `?*T` unless its header says `_Nonnull` (decision 681): glibc's `__attribute__((nonnull))`
  and GLib's comments say it otherwise, and a call passes `.some(p)`.
- `gtk.h` binds only through a header that includes it (`#include <gtk/gtk.h>`): given as the
  header itself, GTK's include guard (`__GTK_H_INSIDE__`) refuses it.

## Language and std

- `System.env` exists on the C library's runtime only (decision 676): the kernel, WASI and
  Windows runtimes set no variable. Their environments are the process's initial stack
  (`envp`), the WASI `environ_get` calls and `GetEnvironmentVariableW`.
- The pool heap of those runtimes (decision 683) gives pages back only for requests past 2048
  bytes; the blocks it cuts from its chunks are reused, never returned.
