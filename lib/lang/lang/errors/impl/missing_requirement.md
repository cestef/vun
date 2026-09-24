# impl.missing_requirement

An impl does not provide a requirement of its interface.

An interface is a function from a type to a struct of requirements, the declarations whose value
is `_`. `#[impl(I(T))]` checks the struct it annotates against the requirements of `I(T)` and fills
in the defaults; every requirement without a default must be provided (spec 02, section 3).

## Example

```ov
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
}});

Sink := struct {};

#[impl(Writer(Sink))] //~ impl.missing_requirement
SinkWriter := struct {
    Error := ();
};
```

## Fix

Provide the missing requirement, `write`:

```ov
Writer := interface(($Self: type) -> type { struct {
    Error: type = _;
    write: (self: &mut Self, bytes: &[u8]) -> Result(usize, Error) = _;
}});

Sink := struct {};

#[impl(Writer(Sink))]
SinkWriter := struct {
    Error := ();
    write := (self: &mut Sink, bytes: &[u8]) -> Result(usize, Error) { .ok(bytes.len()) };
};
```
