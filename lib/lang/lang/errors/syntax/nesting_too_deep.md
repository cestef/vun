# syntax.nesting_too_deep

Expressions or blocks nest deeper than the parser allows.

The grammar places no bound on nesting (appendix D), but the parser does: past 256 levels it
reports this diagnostic and skips the operand, so that no input can exhaust its stack. Each operand
and each block is one level, so the literal below, inside 256 parentheses, is at level 257. Source
this deep is almost always generated.

## Example

```vun
x :=
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    (((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((( //~ syntax.nesting_too_deep
    1
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))));
```

## Fix

Drop the redundant nesting, or split the expression into named bindings:

```vun
x := 1;
```
