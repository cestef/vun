# place.linear_leak

A linear value is still held when its scope exits.

Linear values must be consumed exactly once (spec 05, section 1). At every scope exit, every place
holding a linear value must be `uninit`: forgetting to close, free, release or finish something is
an error, and no destructor ever runs implicitly (spec 05, section 2). `defer` consumes a value on
every exit where it is still held (spec 05, section 3).

## Example

```vun
File := #[kind.linear] struct { len: u64 };

#[link.symbol("file_open")]
open: (path: &[u8]) -> File = _;
#[link.symbol("file_close")]
close: (f: File) = _;

size := (path: &[u8]) -> u64 {
    f := open(path);
    f.len //~ place.linear_leak
};
```

## Fix

Consume the value before the scope exits, here by closing the file:

```vun
File := #[kind.linear] struct { len: u64 };

#[link.symbol("file_open")]
open: (path: &[u8]) -> File = _;
#[link.symbol("file_close")]
close: (f: File) = _;

size := (path: &[u8]) -> u64 {
    f := open(path);
    len := f.len;
    close(f);
    len
};
```
