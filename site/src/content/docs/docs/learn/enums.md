---
tableOfContents: false
title: "Enums"
sidebar:
  order: 6
lesson: {"task": "`empty` was just added. Run it to see what the `match` forgot, and handle it: an empty shape has no area.", "hint": "A variant without data is matched as `.empty`. What is its area?", "expect": {"output": ".circle(5) has area 75\n.rect(3, 4) has area 12\n.tri(6, 5) has area 15\n.empty has area 0\n"}}
---

An enum is one of several variants, and a variant can carry data. A `match` has to cover every variant, so adding one makes the compiler walk you through each `match` that needs it.
