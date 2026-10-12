# layout.code

The tags an enum's variants name (`name = code`) cannot be stored: one variant holds a value, two
share a tag, or a tag is outside the integer that stores them.

A variant of an enum may name its tag, as C's enums do: `fill = 0, start = 1`; one that does not
has the tag after the variant before it, 0 for the first (spec 05, section 6). The tags are the
integer `#[layout.tag(T)]` gives, or the least integer holding every one. An enum that names tags
holds no values in its variants, and no two variants have the same tag.

## Example

```vun
Align := enum { fill = 0, start = 1, end = 1 }; //~ layout.code
```

## Fix

Give each variant a tag of its own, and keep them within the tag type:

```vun
Align := enum { fill = 0, start = 1, end = 2 };
```
