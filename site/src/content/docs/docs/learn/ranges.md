---
tableOfContents: false
title: "Integers are ranges"
sidebar:
  order: 4
lesson: {"task": "Run it and read the error, then make it build and print `400`.", "hint": "The result can reach 510: give `double` a result type that holds every value.", "expect": {"output": "400\n"}}
---

Every integer type is a range: `u8` is `Int(0..=255)`. Arithmetic is exact, so `a * 2` is `Int(0..=510)` and can never overflow. What the compiler will not do is squeeze that into a `u8` for you.
