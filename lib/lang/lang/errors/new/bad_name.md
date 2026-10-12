# new.bad_name

`vun new` was asked to start a project in a directory whose name is no name in code.

A package is named by its directory, and code reaches it by that name (`gtk_todo.model`), so the
name is one a program can write: letters, digits and `_`, starting with a letter or `_`. A `-`, a
space or a dot cannot be written there.

## Example

```console
$ vun new gtk-todo
error[new.bad_name]: `gtk-todo` cannot name a package
  help: name the directory `gtk_todo`
```

## Fix

Name the directory with a name the language can write:

```console
$ vun new gtk_todo
    Creating gtk_todo
```
