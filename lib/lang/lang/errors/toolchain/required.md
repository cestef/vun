# toolchain.required

A component cannot be removed: another installed component needs it.

A component names the components it cannot work without (`needs`), and installing it installs
them (spec 12, section 6). Removing one of them would leave the other unable to load.

## Example

```console
$ vun component remove backend-native
error[toolchain.required]: `backend-native` is needed by `backend-cranelift`
```

## Fix

Remove the component that needs it first, or both at once:

```console
$ vun component remove backend-cranelift backend-native
```
