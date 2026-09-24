# binding.hole_value

`_` is used as a whole value where nothing provides it.

`name: T = _;` declares a binding whose value is not in the source (spec 01, section 3). It is
allowed in three places only: inside `lang`, where the compiler provides it; with a `link`
attribute, where the linker provides it; and inside an interface, where the conforming `impl`
provides it (spec 02, section 3). Anywhere else, `_` as a value is an error.

## Example

```vun
answer: u32 = _; //~ binding.hole_value
```

## Fix

Give the binding its value, or name the symbol the linker provides it from:

```vun
#[link.symbol("answer")]
answer: u32 = _;
```
