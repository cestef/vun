---
tableOfContents: false
title: "The world is a value"
sidebar:
  order: 14
lesson: {"task": "Make the note say `buy bread` (keep the newline) and run it.", "hint": "The text is the string that `std.fmt.print(note.stream(), ...)` writes.", "expect": {"output": "10 bytes: buy bread\ngone.txt is missing\n"}}
---

Files, clocks, standard output: all of it hangs off `sys`, a value you pass on, so a function that is not given it cannot touch the world. Failure is a value too: a file that is not there is `.missing`, which you can match.
