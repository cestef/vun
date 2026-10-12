# todo

A to-do list in GTK 4, written in Vun: add, edit, delete, check off, filter (all, active, done),
clear done, and the list kept in `~/.local/share/todo.json` (`todo.json` in the working directory
when `HOME` is not set).

Needs `gtk4-devel` (Fedora) or its equivalent, and the repository's `vun`, since it uses the
library of this tree:

    vun run
    vun test

The package builds for `linux.x86_64.gnu` (`build.vun`), the target on a C library, and links
GTK because each import in `todo/gtk/sys.vun` names its library (`#[link.library]`).

`todo/gtk/sys.vun` is bound from GTK's headers by `vun util ffi bind`: `./bind.sh` writes it again,
asking for the functions the program calls and the types they use.

| Path | Holds |
|------|-------|
| `todo/model/` | items, filters, the list; tests |
| `todo/store/` | the JSON document, read and written; tests |
| `todo/gtk/` | typed handles over the bound C functions of `todo/gtk/sys.vun` |
| `todo/ui/` | state, window, rows, signal handlers |
