# names.visibility

A `#[visible(...)]` attribute cannot apply where it is written.

`#[visible(scope)]` widens where a restricted name, one starting with `_`, is visible: from the
namespace that declares it to `scope` (spec 01, section 7). It applies only to restricted names,
since widening never makes a name public, and `.within(ns)` must name a namespace that encloses the
declaration, since a scope is always around what it holds.

## Example

```vun
#[visible(.package)] //~ names.visibility
limit := 64;
```

## Fix

Start the name with `_` to restrict it, or drop the attribute to keep it public:

```vun
#[visible(.package)]
_limit := 64;
```
