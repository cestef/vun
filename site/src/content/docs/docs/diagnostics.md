---
title: "Errors and fixes"
description: "Read a diagnostic, explain it, let the compiler fix it."
---

Every error, warning and lint has an id such as `names.duplicate`. The id is stable, it has a page of its own in the compiler, and some of them come with a fix.

## Read a diagnostic

A program that declares a name twice:

```ansi
    [1m[32mChecking[0m [1mhello[0m
[1m[91merror[names.duplicate][0m[1m: `answer` is declared twice in this namespace[0m
 [1m[94m ╭▸ [0mmain.vun:4:1
  [1m[94m│[0m
[1m[94m3[0m [1m[94m│[0m answer := 6 * 7;
  [1m[94m│[0m [1m[94m──────[0m [1m[94mfirst bound here[0m
[1m[94m4[0m [1m[94m│[0m answer := 1;
  [1m[94m│[0m [1m[91m━━━━━━[0m
  [1m[94m│[0m
  [1m[94m╰ [0m[1mhelp[0m: choose another name

      [1m[91mFailed[0m [1mhello[0m with [1m[91m1 error[0m
```

The label under the thick line marks the error, thin lines mark related places, and `help` says what to try. The position points into your source.

## Explain it

```sh
vun explain names.duplicate
```

prints the page for the id: what the rule is, an example that triggers it, and a fix. Accept the short form (`names.duplicate`) or the full one (`lang.errors.names.duplicate`).

## Check without building

```sh
vun check
```

analyzes the package and stops before code generation, which is the quick loop while you edit. `--widen` lists arithmetic intermediates wider than the target's register, and `--meta-stats` reports the instantiations and code size of each generic function the program reaches.

## Let the compiler fix it

```sh
vun fix
```

applies the fixes diagnostics propose, then reports what is left. A fix that may change what the program does is only applied with `--unsafe`.

```ansi
    [1m[32mChecking[0m [1mhello[0m
[1m[91merror[syntax.expected_token][0m[1m: expected `;` or `}`, found `std`[0m
 [1m[94m ╭▸ [0mmain.vun:6:28
  [1m[94m│[0m
[1m[94m6[0m [1m[94m│[0m     mut out := sys.stdout()
  [1m[94m╰╴[0m                           [1m[91m━[0m

```

Not every error has a fix. This one does not (the missing `;` is a guess), so `vun fix` reports it and leaves the file alone.

## Format

```sh
vun fmt
vun fmt --check src/
```

`vun fmt` rewrites files in the layout the formatter gives them. With `--check` it changes nothing and fails when a file would change, which is what CI wants. `--width` sets the most columns a line has where it can be broken.

## Lints

Lints are diagnostics a package can raise or lower. Conventions such as naming are lints that are on by default, and each is configurable:

```sh
vun check --lint std.lints.naming.case=warn
```
