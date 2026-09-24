# syntax.unknown_character

A character that starts no token of the grammar.

Every token is declared in `lang/syntax/grammar.vun` (spec 13, section 5); the symbols of the
language and their meanings are listed in spec 01, section 2. A run of characters that matches none
of them is reported as not a valid token, up to the next character that can start one.

## Example

```vun
a := 1;
b := 2;
total := a @ b; //~ syntax.unknown_character
```

## Fix

Use an operator of the grammar:

```vun
a := 1;
b := 2;
total := a + b;
```
