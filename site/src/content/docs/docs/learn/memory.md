---
tableOfContents: false
title: "Memory"
sidebar:
  order: 12
lesson: {"task": "The 64 bytes run out after 8 squares. Make the buffer big enough for all 20.", "hint": "A `u32` is 4 bytes, and while a `Vec` grows it briefly needs the old buffer and the new one. Try 256.", "expect": {"output": "[0, 1, 4, 9, 16, 25, 36, 49, 64, 81, 100, 121, 144, 169, 196, 225, 256, 289, 324, 361]\n"}}
---

Nothing allocates unless you hold an allocator, and an allocator is a value. `Fixed` hands out bytes of a buffer you own, a `Vec` grows through whichever one you pass it, and running out is an error you can read, not a crash. Nothing is freed behind your back: `deinit` is yours to call.
