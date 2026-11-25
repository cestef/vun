---
tableOfContents: false
title: "Interfaces"
sidebar:
  order: 10
lesson: {"task": "Make `{}` print a `Point` as `(3, -4)`: write the body of `format`, reading `value.x` and `value.y`.", "hint": "`std.fmt.print(w, \"...\", ...)` writes to the writer `w` it was given, and gives back the `Result` `format` returns.", "expect": {"output": "Point{ .x = 3, .y = -4 }\n(3, -4)\n"}}
---

An interface is a set of functions a type provides, and `impl I for T { }` is how it joins in. `{}` asks for `std.fmt.Format`, which `std` declares in vun like anything else; `{?}` asks for `Debug`, which comes from a struct's fields when you write nothing.
