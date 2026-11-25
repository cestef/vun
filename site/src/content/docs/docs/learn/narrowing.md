---
tableOfContents: false
title: "Narrowing"
sidebar:
  order: 5
lesson: {"task": "The last line traps. Run it, then make the program end normally.", "hint": "Put the line that traps under `if big < 256 { }`.", "expect": {"exit": 0}}
---

Narrowing is the only way a number can go wrong, so you always write it: `to` traps when the value does not fit, `wrap` is modulo, `sat` clamps, and `try_to` gives an optional. A comparison tells the compiler something too: inside `if big < 256`, `big` is known to fit.
