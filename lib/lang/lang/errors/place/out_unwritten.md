# place.out_unwritten

A function returns without initializing the place of an `&out` borrow.

An `&out` borrow takes an `uninit` place and leaves it `init`: the callee must initialize it on
every path that returns (spec 05, section 2). The caller relies on it, reading the place after the
call without any check.

## Example

```ov
Header := struct { magic: u32, len: u16 };

read := (bytes: &[6]u8, h: &out Header) {
    if bytes[0] == 0 { return; } //~ place.out_unwritten
    h.* = .{ .magic = 1, .len = 2 };
};
```

## Fix

Write the place before every return:

```ov
Header := struct { magic: u32, len: u16 };

read := (bytes: &[6]u8, h: &out Header) {
    if bytes[0] == 0 {
        h.* = .{ .magic = 0, .len = 0 };
        return;
    }
    h.* = .{ .magic = 1, .len = 2 };
};
```
