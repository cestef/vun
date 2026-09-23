# syntax.invalid_escape

A backslash in a string or character literal starts no valid escape.

The escapes are `\n \t \r \0 \\ \" \'`, `\xHH` and `\u{H..HHHHHH}` (spec 01, section 5); the
diagnostic lists them. Any other character after `\` inside `"..."` or `'...'` is an error, which
keeps a mistyped path or pattern from silently meaning something else.

## Example

```ov
path := "C:\dir"; //~ syntax.invalid_escape
```

## Fix

Take the string literally with a leading backslash, so it has no escapes:

```ov
path := \"C:\dir";
```
