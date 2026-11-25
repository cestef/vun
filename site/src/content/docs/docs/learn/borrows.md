---
tableOfContents: false
title: "Borrows"
sidebar:
  order: 8
lesson: {"task": "The two `&mut` borrows of `n` overlap. Reorder the lines so it builds and prints `3`.", "hint": "Finish with `a` before you borrow `n` again.", "expect": {"output": "3\n"}}
---

`&x` borrows, `&mut x` borrows exclusively, and `.*` reaches what a borrow points at. A borrow lasts until its last use, and while an exclusive one is live nothing else can reach the place. There are no lifetime annotations: the compiler reads your code.
