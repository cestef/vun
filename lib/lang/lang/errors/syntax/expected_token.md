# syntax.expected_token

The parser needs a specific token, such as `;`, `,` or `=>`, and finds another.

Every statement ends with `;`, except a block-like expression used as a statement, and list items
are separated by `,` (spec 01, section 6); a match arm needs `=>` (appendix D). The diagnostic
names the tokens that would fit and is placed right after the previous token, where the missing one
belongs.

## Example

```ov
limit := 64 //~ syntax.expected_token
count := 0;
```

## Fix

Insert the expected token:

```ov
limit := 64;
count := 0;
```
