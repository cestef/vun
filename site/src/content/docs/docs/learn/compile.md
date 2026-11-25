---
tableOfContents: false
title: "Compile time"
sidebar:
  order: 13
lesson: {"task": "21! does not fit a `u64`, and the compiler finds out before the program exists. Ask for 20.", "hint": "Read the error: it points at the `$` call.", "expect": {"output": "3628800\n2432902008176640000\n"}}
---

`$` means one stage earlier. `$fact(20)` runs `fact` while compiling, and the program only holds the result. Compile time and run time share one semantics, so an overflow is the same trap, reported as a compile error.
