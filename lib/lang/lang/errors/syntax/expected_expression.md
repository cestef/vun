# syntax.expected_expression

An expression is required where the parser finds none.

An operator takes its operands, a binding takes its value, and every item of a list is an
expression (appendix D). The diagnostic is placed where the expression belongs. A keyword is never
an expression; when one stands where a name was meant, the fix escapes it (`\unsafe`, spec 01,
section 1).

## Example

```ov
limit := 64;
twice := limit * ; //~ syntax.expected_expression
```

## Fix

Write the missing operand:

```ov
limit := 64;
twice := limit * 2;
```
