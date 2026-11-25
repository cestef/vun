---
tableOfContents: false
title: "Errors"
sidebar:
  order: 7
lesson: {"task": "`5000` is a number, only a big one. Make it `.err(.too_big)` instead of `.err(.not_a_number)`.", "hint": "Parse into a wider range, then compare: after `if n > 1000 { return ... }` the compiler knows `n` is small enough for the `u16`.", "expect": {"output": ".ok(42)\n.err(.empty)\n.err(.not_a_number)\n.err(.too_big)\n"}}
---

There are no exceptions. A function that can fail returns a `Result`, and a trailing `?` hands the error straight back to the caller. Errors are plain enums, so you match on them, or print them with `{?}`.
