# syntax.stray_escape

A backslash outside a literal is followed by neither a word nor a string.

A backslash takes what follows literally (spec 01, section 2): `\unsafe` is the name `unsafe`
(spec 01, section 1), `\"C:\dir"` is a string without escapes and `\\` starts a raw line. The
backslash must touch what it escapes; alone, it means nothing.

## Example

```ov
Mode := enum { safe, \ unsafe }; //~ syntax.stray_escape
```

## Fix

Write the backslash directly before the word:

```ov
Mode := enum { safe, \unsafe };
```
