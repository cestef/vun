# syntax.unterminated_string

A string literal reaches the end of its line without a closing `"`.

Strings end on the line they start (spec 01, section 5). This holds for escaped strings `"..."`
and for literal strings `\"..."`; text that spans lines is written as a run of lines each starting
with `\\`.

## Example

```ov
greeting := "hello; //~ syntax.unterminated_string
```

## Fix

Close the string on the same line:

```ov
greeting := "hello";
```
