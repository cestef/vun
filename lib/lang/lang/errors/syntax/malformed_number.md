# syntax.malformed_number

A number literal holds characters that do not belong to it.

Integers are written `1_000`, `0xff`, `0o17` and `0b1010`, and floats `1.5` and `1e-3` (spec 01,
section 5). Digits outside the literal's radix, letters glued to its end, or a radix prefix with no
digits (`0x`) make the whole literal malformed.

## Example

```ov
mask := 0b1021; //~ syntax.malformed_number
```

## Fix

Write only digits of the literal's radix:

```ov
mask := 0b1011;
```
