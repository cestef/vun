# syntax.expected_declaration

A file or struct body holds something that is not a declaration.

A file is a struct whose members are its top-level declarations (spec 01, section 8), and every
declaration is a binding (spec 01, section 3). Statements such as calls belong in function bodies.
A doc comment or an attribute attaches to the declaration after it, so one with no declaration
following is reported too.

## Example

```ov
limit := 64;
/// The largest frame, in bytes. //~ syntax.expected_declaration
```

## Fix

Place the doc comment before the declaration it documents:

```ov
/// The largest frame, in bytes.
limit := 64;
```
