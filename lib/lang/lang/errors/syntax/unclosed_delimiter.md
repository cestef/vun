# syntax.unclosed_delimiter

A `(`, `[` or `{` has no matching closer.

Lists are delimited and comma-separated (spec 01, section 6), and blocks are braced. The diagnostic
is reported at the opener and says which closer was expected and what was found instead. An
argument list left open ends at the statement's `;`; a body left open ends at the end of the file.

## Example

```ov
run := (x: u8, y: u8) {};
a := 1;
b := 2;

main := () {
    run(a, b; //~ syntax.unclosed_delimiter
};
```

## Fix

Close the group before the `;`:

```ov
run := (x: u8, y: u8) {};
a := 1;
b := 2;

main := () {
    run(a, b);
};
```
