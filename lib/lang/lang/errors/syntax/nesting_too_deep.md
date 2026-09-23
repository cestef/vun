# syntax.nesting_too_deep

Expressions or blocks nest deeper than the parser allows.

The grammar places no bound on nesting (appendix D), but the parser does: past 256 levels it
reports this diagnostic and skips the operand, so that no input can exhaust its stack. Source this
deep is almost always generated.

## Example

```ov
x :=
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    ((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((((
    1 //~ syntax.nesting_too_deep
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))
    ))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))))));
```

## Fix

Drop the redundant nesting, or split the expression into named bindings:

```ov
x := 1;
```
