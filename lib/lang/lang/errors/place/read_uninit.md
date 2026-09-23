# place.read_uninit

A place is read while it may be `uninit`.

Every place is `uninit` or `init`, and reading requires `init` (spec 05, section 2). A declaration
without a value, such as `mut buf: [16]u8;`, costs nothing and there is no `undefined` value, so
the place must be written before it is read. Passing a linear value by value moves it and leaves
its place `uninit`. The check runs on the control-flow graph.

## Example

```ov
first := () -> u8 {
    mut buf: [16]u8;
    buf[0] //~ place.read_uninit
};
```

## Fix

Initialize the place before reading it:

```ov
first := () -> u8 {
    buf: [16]u8 = [0; 16];
    buf[0]
};
```
