---
tableOfContents: false
title: "Types are values"
sidebar:
  order: 9
lesson: {"task": "Print the larger of `-5` and `-2` as well, with the same `larger`.", "hint": "Make another pair, of `i32`, and pass it to `larger`: it is read from the pair which type it works on.", "expect": {"output": "Pair(u8){ .a = 7, .b = 42 }\n42\n-2\n"}}
---

A type is a value, so a function can take one and return one: `Pair(u8)` is an ordinary call. `$` marks what has to be known while compiling. That is all generics are: there is no separate feature.
