---
tableOfContents: false
title: "Iterators"
sidebar:
  order: 11
lesson: {"task": "Yield only the even numbers.", "hint": "`yield` is a statement like any other. Put it under an `if`.", "expect": {"output": "2\n4\n6\n8\n10\n"}}
---

A function with `yield` is a coroutine: calling it gives something `for` can loop over, and `yield` hands a value out and pauses on the spot. Its state is a frame of a fixed size, so there is no heap and no allocator involved.
