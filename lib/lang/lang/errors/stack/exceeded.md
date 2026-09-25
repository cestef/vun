# stack.exceeded

A function bound by `#[stack.max(n)]` may take more than `n` bytes of stack.

The bound covers the function's own frame and the deepest chain of calls it makes, over the
whole program's call graph (spec 07, section 4). Frame sizes are those the backend reports, so
the same program may fit under one backend and not another; the check runs when the program is
built.

## Example

```console
$ cat fill.vun
#[link.export("fill"), abi.c, stack.max(64)]
fill := (seed: u8) -> u8 {
    buf: [256]u8 = [seed; 256];
    copy := buf;
    seed
};
$ vun build --backend baseline fill.vun
error[stack.exceeded]: this function may take 1056 bytes of stack, over its bound of 64
```

## Fix

Raise the bound, or move the large locals out of the frame, into a static or storage the caller
lends:

```console
$ cat fill.vun
#[link.export("fill"), abi.c, stack.max(64)]
fill := (buf: &mut [256]u8, seed: u8) -> u8 { seed };
$ vun build --backend baseline fill.vun
```
