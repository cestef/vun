# tool.disasm_failed

`vun debug disasm` could not list the code of a file.

The command reads an ELF or PE file and lists the instructions of its executable sections,
labelled by its symbols (a PE file's exports, entry and imports). It stops where the file is
neither, where it is malformed, where its machine is one whose instructions are not read (x86,
x86-64 and arm64 are), or where no code is labelled by the symbol it was asked for.

## Example

```console
$ vun debug disasm app missing
error[tool.disasm_failed]: no code is labelled `missing`
```

## Fix

Name a symbol of the file, which `nm` or a listing without a name shows:

```console
$ vun debug disasm app
$ vun debug disasm app main
```
