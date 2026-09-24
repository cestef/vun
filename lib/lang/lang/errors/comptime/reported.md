# comptime.reported

Library code reported an error while it was evaluated at compile time.

A library checks what only it knows how to check, such as a format string against its arguments or
a register map against the device, by calling `std.diag.error` in code evaluated at compile time
(spec 10, section 3). Evaluation stops there, and the error is shown where the value being computed
was needed, with each call it was reported in. The message is the library's.

## Example

```vun
checked := ($n: u8) -> u8 {
    $if n > 9 { $std.diag.error(.{ .msg = "one digit at most" }); }
    n
};
digit := checked(12); //~ comptime.reported
```

## Fix

Do what the message asks; here, pass a value the library accepts:

```vun
checked := ($n: u8) -> u8 {
    $if n > 9 { $std.diag.error(.{ .msg = "one digit at most" }); }
    n
};
digit := checked(7);
```
