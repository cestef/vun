# meta.escape

A `$` has no stage to escape to.

Each `$` escapes one level: out of the innermost quote around it, or, outside every quote, from
run time to compile time. A chain of `$` inside `n` quotes can escape at most `n + 1` levels
(spec 03, section 4).

## Example

```vun
twice := ($x: Code(u32)) -> Code(u32) { #{ $$$x } }; //~ meta.escape
```

## Fix

Escape only as far as the stages around the code:

```vun
twice := ($x: Code(u32)) -> Code(u32) { #{ $x } };
```
