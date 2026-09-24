# syntax.unterminated_char

A character literal lacks its closing `'`.

A character literal `'a'` holds one character, or one escape, between single quotes, and is an
integer literal of its Unicode scalar value (spec 01, section 5).

## Example

```vun
newline := '\n; //~ syntax.unterminated_char
```

## Fix

Close the literal after its character:

```vun
newline := '\n';
```
