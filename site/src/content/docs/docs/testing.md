---
title: "Testing"
description: "Write tests as declarations and run them with `vun test`."
---

A test is a declaration with the `#[test]` attribute. It lives in an ordinary source file, usually `<package>/tests.vun`, and `vun test` finds it.

## Write one

```vun
#[test]
adds := () { std.testing.assert(1 + 1 == 2); };
```

`std.testing.assert` traps when its condition is false, and a trapped test fails.

## Run them

```sh
vun test
```

```ansi
      [1m[32mPassed[0m [1madds[0m[2m 61.39 µs[0m
      [1m[32mTested[0m 1 passed[2m in 83.16 µs[0m
```

A failing test reports the assertion, its position and the stack of the trap, and the run exits with an error:

```ansi
     [1m[91mTrapped[0m [1mwrong[0m[2m 646.89 µs[0m
             [1m[91massertion failed: `1 + 1 == 3`[0m
             [34mat hello/tests.vun:7:34[0m
             trap: assertion failed: `1 + 1 == 3`
               at hello/tests.vun:7:34
               in wrong (hello/tests.vun:7:10)
      [1m[91mTested[0m 1 passed, 1 trapped[2m in 1.22 ms[0m
```

Each test runs in a process of its own, so one that traps does not take the others with it. `--jobs` (`-j`) sets how many run at once, and `--jobs 1` runs them in one process, in order.

## Pick tests

```sh
vun test adds            # by name
vun test --filter add    # by a part of the name
```

## Properties

A property test runs with many generated cases. `--cases` sets how many (100 when not given), and `--seed` draws them from the seed a falsified property reported, so a failure is reproducible.

## Beyond tests

| Command | Does |
|---|---|
| `vun cover` | run the tests and report the lines they reach |
| `vun fuzz` | fuzz the properties, guided by what their cases cover |
| `vun bench` | run the benchmarks of a namespace, measured |
