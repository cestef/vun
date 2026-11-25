---
tableOfContents: false
title: "Hello"
sidebar:
  order: 1
lesson: {"task": "Make it print `the answer is 42`, then run it.", "hint": "6 * 9 is 54. Bind `answer` to something else.", "expect": {"output": "the answer is 42\n"}}
---

Everything in vun is a binding: `name := value`. Even `main` is one, bound to a function. It is handed `sys`, the program's way out to the world: nothing is ambient, so printing means asking `sys` for its standard output.
