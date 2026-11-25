---
tableOfContents: false
title: "Printing"
sidebar:
  order: 2
lesson: {"task": "Make the last line show `n` in binary, 8 digits wide, padded with zeros: `11001010`.", "hint": "The mark is `b`. A fill and a width go after the colon, as in `{x:0>4}`: pad with `0`, on the left, to 4.", "expect": {"output": "decimal: 202\nhex:     ca\ndebug:   .some(202)\npadded:  [   202]\nbinary:  11001010\n"}}
---

The format string is read while compiling, so a mistake in it is an error before the program exists. `{}` writes a value the usual way; `{x}` writes it in hex, `{b}` in binary and `{?}` as the code that builds it. After a `:` goes where it sits: a fill, `<`, `^` or `>`, and a width.
