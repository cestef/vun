---
title: "Your first program"
description: "Make a project, run it, break it, read what the compiler says."
---

## Make a project

```sh
vun new hello
cd hello
```

`vun new` makes a directory named after the package and fills it from a template. `hello` is the default template, a hosted program. The other one is `rp2350`, firmware for a Raspberry Pi Pico 2 (`vun new blink --template rp2350`).

The project has four files:

| File | Holds |
|---|---|
| `build.vun` | the package record and the build |
| `main.vun` | the program |
| `hello.vun` | the package namespace |
| `hello/tests.vun` | the tests `vun test` runs |

## Run it

```sh
vun run
```

```ansi
hello
    [1m[32mBuilding[0m [1mhello[0m
    [1m[32mFinished[0m [1m.vun/build/main[0m in 0.42s
     [1m[32mRunning[0m [1m.vun/build/main[0m
```

Builds and their cached queries live in `.vun/` beside the project.

## Read it

```vun run
start := std.os.start(main);

main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "hello\n");
    .ok
};
```

Everything in Vun is a binding: `name := value`. Even `main` is one, bound to a function. `start` binds the runtime, which is a function you call with your `main`, so no name is special to the compiler.

Nothing is ambient. `main` is handed `sys`, the program's way out to the world, and printing means asking `sys` for its standard output. A function that is not given `sys` cannot print, open a file or reach the network.

`.ok` is an enum literal: the variant `ok` of the type the function returns, `std.os.Exit`.

## Break it

Declare a name twice and the compiler refuses before anything runs.

```vun error
start := std.os.start(main);

answer := 6 * 7;
answer := 1;

main := (sys: &mut std.os.System) -> std.os.Exit {
    mut out := sys.stdout();
    std.fmt.print(&mut out, "{}\n", answer);
    .ok
};
```

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

The id in brackets names the diagnostic. [Errors and fixes](../diagnostics/) shows what to do with it.

## Keep going

The [course](../../learn/) teaches the language by having you fix small programs in the browser, and the [playground](../../play/) runs anything you paste. For the design behind the language, read the [specification](https://github.com/cestef/vun/blob/main/docs/spec/00-principles.md).
