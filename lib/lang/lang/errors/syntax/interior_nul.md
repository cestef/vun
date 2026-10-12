# syntax.interior_nul

A string for C, written `c"..."`, holds a zero byte before its end.

A string for C is the bytes between the quotes followed by one zero byte, which is where C ends
the string (spec 01, section 5). A zero byte inside it, written `\0`, `\x00` or `\u{0}`, would end
it early for every C function that reads it.

## Example

```vun
name := c"a\0b"; //~ syntax.interior_nul
```

## Fix

Remove the zero byte, or write the bytes as a string without `c`:

```vun
name := c"ab";
```
