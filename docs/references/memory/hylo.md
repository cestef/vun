For the complete documentation index, see [llms.txt](https://docs.hylo-lang.org/language-tour/llms.txt). This page is also available as [Markdown](https://docs.hylo-lang.org/language-tour/functions-and-methods.md).

![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNDQ4IDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gYnV0dG9uLWxlYWRpbmctaWNvbiBzaHJpbmstMCBteS10ZXh0LTEvOCBzaXplLXRleHQtYmFzZSI+PHBhdGggZmlsbD0iY3VycmVudENvbG9yIiBkPSJNMzg0IDMzNmwtMTkyIDBjLTguOCAwLTE2LTcuMi0xNi0xNmwwLTI1NmMwLTguOCA3LjItMTYgMTYtMTZsMTMzLjUgMGM0LjIgMCA4LjMgMS43IDExLjMgNC43bDU4LjUgNTguNWMzIDMgNC43IDcuMSA0LjcgMTEuM0w0MDAgMzIwYzAgOC44LTcuMiAxNi0xNiAxNnpNMTkyIDM4NGwxOTIgMGMzNS4zIDAgNjQtMjguNyA2NC02NGwwLTE5Ny41YzAtMTctNi43LTMzLjMtMTguNy00NS4zTDM3MC43IDE4LjdDMzU4LjcgNi43IDM0Mi41IDAgMzI1LjUgMEwxOTIgMGMtMzUuMyAwLTY0IDI4LjctNjQgNjRsMCAyNTZjMCAzNS4zIDI4LjcgNjQgNjQgNjR6TTY0IDEyOGMtMzUuMyAwLTY0IDI4LjctNjQgNjRMMCA0NDhjMCAzNS4zIDI4LjcgNjQgNjQgNjRsMTkyIDBjMzUuMyAwIDY0LTI4LjcgNjQtNjRsMC0xNi00OCAwIDAgMTZjMCA4LjgtNy4yIDE2LTE2IDE2TDY0IDQ2NGMtOC44IDAtMTYtNy4yLTE2LTE2bDAtMjU2YzAtOC44IDcuMi0xNiAxNi0xNmwxNiAwIDAtNDgtMTYgMHoiIC8+PC9zdmc+)Copy

![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNDQ4IDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gZ3JvdXAtYXJpYS1leHBhbmRlZC9kcm9wZG93bjpyb3RhdGUtMTgwIGdyb3VwLWRhdGEtcGFuZWwtb3Blbi9kcm9wZG93bjpyb3RhdGUtMTgwIGdyb3VwLW9wZW46cm90YXRlLTE4MCBzaXplLTMgdHJhbnNpdGlvbi1hbGwgYnV0dG9uLWxlYWRpbmctaWNvbiBzaHJpbmstMCBteS10ZXh0LTEvOCBzaXplLXRleHQtYmFzZSBteC10ZXh0LTEvOCBzaXplLXRleHQtc20iPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIwNy41IDQwOWM5LjQgOS40IDI0LjYgOS40IDMzLjkgMGwyMDAtMjAwYzkuNC05LjQgOS40LTI0LjYgMC0zMy45cy0yNC42LTkuNC0zMy45IDBsLTE4MyAxODMtMTgzLTE4M2MtOS40LTkuNC0yNC42LTkuNC0zMy45IDBzLTkuNCAyNC42IDAgMzMuOWwyMDAgMjAweiIgLz48L3N2Zz4=)

![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNDQ4IDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gYnV0dG9uLWxlYWRpbmctaWNvbiBzaHJpbmstMCBteS10ZXh0LTEvOCBzaXplLXRleHQtYmFzZSI+PHBhdGggZmlsbD0iY3VycmVudENvbG9yIiBkPSJNMjQgNjRDMTAuNyA2NCAwIDc0LjcgMCA4OHMxMC43IDI0IDI0IDI0bDQwMCAwYzEzLjMgMCAyNC0xMC43IDI0LTI0cy0xMC43LTI0LTI0LTI0TDI0IDY0ek0xNTIgMjMyYy0xMy4zIDAtMjQgMTAuNy0yNCAyNHMxMC43IDI0IDI0IDI0bDI3MiAwYzEzLjMgMCAyNC0xMC43IDI0LTI0cy0xMC43LTI0LTI0LTI0bC0yNzIgMHpNMTI4IDQyNGMwIDEzLjMgMTAuNyAyNCAyNCAyNGwyNzIgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtMjcyIDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0ek0wIDQyNGMwIDEzLjMgMTAuNyAyNCAyNCAyNHMyNC0xMC43IDI0LTI0bDAtMTc2YzAtMTMuMy0xMC43LTI0LTI0LTI0UzAgMjM0LjcgMCAyNDhMMCA0MjR6IiAvPjwvc3ZnPg==)On this page

![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNDQ4IDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gYnV0dG9uLWxlYWRpbmctaWNvbiBzaHJpbmstMCBteS10ZXh0LTEvOCBzaXplLXRleHQtYmFzZSBteC10ZXh0LTEvOCI+PHBhdGggZmlsbD0iY3VycmVudENvbG9yIiBkPSJNMjQgNjRDMTAuNyA2NCAwIDc0LjcgMCA4OHMxMC43IDI0IDI0IDI0bDQwMCAwYzEzLjMgMCAyNC0xMC43IDI0LTI0cy0xMC43LTI0LTI0LTI0TDI0IDY0ek0xNTIgMjMyYy0xMy4zIDAtMjQgMTAuNy0yNCAyNHMxMC43IDI0IDI0IDI0bDI3MiAwYzEzLjMgMCAyNC0xMC43IDI0LTI0cy0xMC43LTI0LTI0LTI0bC0yNzIgMHpNMTI4IDQyNGMwIDEzLjMgMTAuNyAyNCAyNCAyNGwyNzIgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtMjcyIDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0ek0wIDQyNGMwIDEzLjMgMTAuNyAyNCAyNCAyNHMyNC0xMC43IDI0LTI0bDAtMTc2YzAtMTMuMy0xMC43LTI0LTI0LTI0UzAgMjM0LjcgMCAyNDhMMCA0MjR6IiAvPjwvc3ZnPg==)

# Functions and methods

Functions are blocks of reusable code that performs a single action, or group of related actions. They are an essential tool for managing the complexity of a program as it grows.

*Note: Though Hylo should not be considered a functional programming language, functions are first-class citizens, and functional programming style is well-supported. In fact, Hylo's mutable value semantics can be freely mixed with purely functional code without eroding that code's local reasoning properties*

### Free functions[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#free-functions)

A function declaration is introduced with the `fun` keyword, followed by the function's name, its signature, and finally its body:

Copy

``` relative
typealias Vector2 = {x: Float64, y: Float64}

fun norm(_ v: Vector2) -> Float64 {
  Float64.sqrt(v.x * v.x + v.y * v.y)
}

public fun main() {
  let velocity = (x: 3.0, y: 4.0)
  print(norm(velocity)) // 5.0
}
```

The program above declares a function named `norm` that accepts a 2-dimensional vector (represented as a pair of `Float64`) and returns its norm, or length.

A function's signature describes its parameter and return types. The return type of a function that does not return any value, like `main` above, may be omitted. Equivalently we can explicitly specify that the return type is `Void`.

A function is called using its name followed by its arguments and any argument labels, enclosed in parentheses. Here, `norm` is called to compute the norm of the vector `(x: 3.0, y: 4.0)` with the expression `norm(velocity)`.

Notice that the name of the parameter to that function is prefixed by an underscore (i.e., `_`), signaling that the parameter is unlabeled. If this underscore were omitted, a call to `norm` would require its argument to be labeled by the parameter name `v`.

You can specify a label different from the parameter name by replacing the underscore with an identifier. This feature can be used to create APIs that are clear and economical at the use site, especially for functions that accept multiple parameters:

Argument labels are also useful to distinguish between different variants of the same operation.

The two `scale` functions above are similar, but not identical. The first accepts a vector as the scaling factor, the second a scalar, a difference that is captured in the argument labels. Argument labels are part of the full function name, so the first function can be referred to as `scale(_,by:)` and the second as `scale(_,uniformly_by:)`. In fact, Hylo does not support type-based overloading, so the *only* way for two functions to share the same base name is to have different argument labels. *Note: many of the use cases for type-based overloading in other languages can best be handled by using method bundles.*

A function with multiple statements that does not return `Void` must execute one `return` statement each time it is called.

To avoid warnings from the compiler, every non-`Void` value returned from a function must either be used, or be explicitly discarded by binding it to `_`.

Function parameters can have default values, which can be omitted at the call site:

*Note: A default argument expression is evaluated at each call site.*

### Parameter passing conventions[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#parameter-passing-conventions)

A parameter passing convention describes how an argument is passed from caller to callee. In Hylo, there are four: `let`, `inout`, `sink` and `set`. In the next series of examples, we will define four corresponding functions to offset this 2-dimensional vector type:

We will also show how Hylo's parameter passing conventions relate to other programming languages, namely C++ and Rust.

#### **`let`** **parameters**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#let-parameters)

Let's start with the `let` convention. Parameter passing conventions are always written before the parameter type:

`let` is the default convention, so the declaration above is equivalent to

`let` parameters are (notionally) passed by value, and are truly immutable. The compiler wouldn't allow us to modify `v` or `delta` inside the body of `offset_let` if we tried:

\[Recall that `&` is simply a marker required by Hylo when a value is mutated\]

Though `v` cannot be modified, `Vector2` is copyable, so we can copy `v` into a mutable variable and modify *that*.

In fact, when it issues the error about `v` being immutable, the compiler will suggest a rewrite equivalent to the one above (and in the right IDE, will offer to perform the rewrite for you).

The compiler also ensures that `v` and `delta` can't be modified by any other means during the call: their values are truly independent of everything else in the program, preventing all possibility of data races and allowing us to reason locally about everything that happens in the body of `offset_let`. It provides this guarantee in part by ensuring that nothing can modify the arguments *passed to* `offset_let` while the function executes which allows arguments to be passed without making any copies.

A C++ developer can understand the `let` convention as *pass by* *`const`* *reference*, but with the additional static guarantee that there is no way the referenced parameters can be modified during the call.

For example, in the C++ version of our function, `v` and `delta` *could* be modified by another thread while `offset_let` executes, causing a data race. For a single-threaded example, just imagine adding a `std::function` parameter that is called in the body; that parameter might have captured a mutable reference to the argument and could (surprisingly!) modify `v` or `delta` through it.

A Rust developer can understand a `let` parameter as a *pass by immutable borrow*, with exactly the same guarantees:

The only difference between an immutable borrow in Rust and a `let` in Hylo is that the language encourages the programmer to think of a `let` parameter as being passed by value.

The `let` convention does not transfer ownership of the argument to the callee, meaning, for example, that without first copying it, a `let` parameter can't be returned, or stored anywhere that outlives the call.

#### **`inout`** **parameters**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#inout-parameters)

The `inout` convention enables mutation across function boundaries, allowing a parameter's value to be modified in place:

Again, the compiler imposes some restrictions and offers guarantees in return. First, arguments to `inout` parameters must be mutable and marked with an ampersand (`&`) at the call site:

*Note: You can probably guess now why the* *`+=`* *operator's left operand is always prefixed by an ampersand:* *the type of* *`Float64.infix+=`* *is* *`(inout Float64, Float64) -> Void`.*

Second, `inout` arguments must be unique: they can only be passed to the function in one parameter position.

The compiler guarantees that the behavior of `target` in the body of `offset_inout` is as though it had been declared to be a local `var`, with a value that is truly independent from everything else in the program: only `offset_inout` can observe or modify `target` during the call. Just as with the immutability of `let` parameters, this independence upholds local reasoning and guarantees freedom from data races.

A C++ developer can understand the `inout` convention as *pass by reference*, with the additional static guarantee of exclusive access through the reference to the referenced object:

In the C++ version of `offset_inout`, as before, the parameters may be accessible to other threads, opening the possibility of a data race. Also, the two parameters can overlap, and again a simple variation on our function is enough to demonstrate why that might be a problem:

A Rust developer can understand an `inout` parameter as a *pass by mutable borrow*, with exactly the same guarantees:

Again, the only difference is one of perspective: Hylo encourages you to think of `inout` parameters as though they are passed by “move-in/move-out,” and indeed the semantics are the same except that no data actually moves in memory.

Just as with `let` parameters, `inout` parameters are not owned by the callee, and their values cannot escape the callee without first being copied.

**The Fine Print: Temporary Destruction**

Although `inout` parameters are required to be valid at function entry and exit, a callee is entitled to do anything with the value of such parameters, including destroying them, as long as it puts a value back before returning:

In the example above, `v.deinit()` explicitly deinitializes the value of `v`, leaving it unbound. Thus, trying to access its value would constitute an error caught at compile time. Nonetheless, since `v` is reinitialized before the function returns, the compiler is satisfied.

*Note: A Rust developer can understand explicit deinitialization as a call to* *`drop`.* *However, explicit deinitialization always consumes the value, even if its type is copyable.*

#### **`sink`** **parameters**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#sink-parameters)

The `sink` convention indicates a transfer of ownership, so unlike previous examples the parameter *can* escape the lifetime of the callee.

Just as with `inout` parameters, the compiler enforces that arguments to `sink` parameters are unique. Because of the transfer of ownership, though, the argument becomes inaccessible in the caller after the callee is invoked.

A C++ developer can understand the `sink` convention as similar in intent to *pass by rvalue reference*. In fact it's more like pass-by-value where the caller first invokes `std::move` on the argument, because ownership of the argument is transferred at the moment of the function call.

In Hylo, the lifetime of a moved-from value ends, rather than being left accessible in an indeterminate state.

A Rust developer can understand a `sink` parameter as a *pass by move*. If the source type is copyable it is as though it first assigned to a unique reference, so the move is forced:

The `sink` and `inout` conventions are closely related; so much so that `offset_sink` can be written in terms of `offset_inout`, and vice versa.

#### **`set`** **parameters**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#set-parameters)

The `set` convention lets a callee initialize an uninitialized value. The compiler will only accept uninitialized objects as arguments to a set parameter.

A C++ developer can understand the `set` convention in terms of the placement new operator, with the guarantee that the storage in which the new value is being created starts out uninitialized, and ends up initialized.

### Methods[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#methods)

A method is a function associated with a particular type, called the **receiver**, on which it primarily operates. Method declaration syntax is the same as that of a free function, except that a method is always declared in the scope of its receiver type, and the receiver parameter is omitted.

The program above declares `Vector2`, a structure with a method, `offset_let(by:)`, which is nearly identical to the similarly named free function we declared in the section on parameter passing conventions. The difference is that its first parameter, a `Vector2` instance, has become implicit and is now named `self`.

For concision, `self` can be omitted from most expressions in a method. Therefore, we can rewrite `offset_let` this way:

A method is usually accessed as a member of the receiver instance that forms its implicit first parameter, a syntax that binds that instance to the method:

When the method is accessed through its type, instead of through an instance, we get a regular function with an explicit self parameter, so we could have made this equivalent call in the marked line above:

As usual, the default passing convention of the receiver is `let`. Other passing conventions must be specified explicitly, just after the closing parenthesis of the method's parameter list. In the following example, `self` is passed `inout`, making this a mutating method:

In a call to an `inout` method like the one above, the receiver expression is marked with an ampersand, to indicate it is being mutated:

#### **Method bundles**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#method-bundles)

When multiple methods have the same functionality but differ only in the passing convention of their receiver, they can be grouped into a single *bundle*.

In the program above, the method `Vector2.offset(by:)` defines three variants, each corresponding to an implementation of the same behavior, for a different receiver convention.

*Note: A method bundle can not declare a* *`set`* *variant as it does not make sense to operate on a receiver that has not been initialized yet.*

At the call site, the compiler determines the variant to apply depending on the context of the call. In this example, the first call applies the `inout` variant as the receiver has been marked for mutation. The second call applies the `let` variant as the structure is reused (without mutation). The third call applies the `sink` variant as the structure is no longer used afterwards.

Thanks to the link between the `sink` and `inout` conventions, the compiler is able to synthesize one implementation from the other. Further, the compiler can also synthesize a `sink` variant from a `let` one.

This feature can be used to avoid code duplication in cases where custom implementations of the different variants do not offer any performance benefit, or where performance is not a concern. For example, in the case of `Vector2.offset(by:)`, it is sufficient to write the following declaration and let the compiler synthesize the missing variants.

#### **Static methods**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#static-methods)

A type can be used as a namespace for global functions that relate to that type. For example, the function `Float64.random(in:using:)` is a global function declared in the namespace of `Float64`.

A global function declared in the namespace of a type is called a *static method*. Static methods do not have an implicit receiver parameter. Instead, they behave just like regular global functions.

A static method is declared with `static`:

When the return type of a static method matches the type declared by its namespace, the latter can be omitted if the compiler can infer it from the context of the expression:

### Closures[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#closures)

Functions are first-class citizens in Hylo, meaning that they be assigned to bindings, passed as arguments or returned from functions, like any other value. When a function is used as a value, it is called a *closure*.

Some methods of the standard library use closures to implement certain algorithms. For example, the type `T[n]` has a method `reduce(into:_:)` that accepts a closure as second argument to describe how its elements should be combined.

*Note: The method* *`Int.infix+=`* *has the same type as* *`combine(_:_:)`* *in this example.* *Therefore, we could have written* *`numbers.reduce(into: 0, Int.infix+=)`.*

When the sole purpose of a function is to be used as a closure, it may be more convenient to write it inline, as a closure expression. Such an expression resembles a function declaration, but has no name. Further, the types of the parameters and/or the return type can be omitted if the compiler can infer those from the context.

#### **Closure captures**[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgNTEyIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2VsZi1jZW50ZXIgdHJhbnNpdGlvbi1jb2xvcnMgdGV4dC10cmFuc3BhcmVudCBwb2ludGVyLWZpbmU6Z3JvdXAtaG92ZXIvaGFzaDp0ZXh0LXRpbnQtc3VidGxlIGNvbnRyYXN0LW1vcmU6cG9pbnRlci1maW5lOmdyb3VwLWhvdmVyL2hhc2g6dGV4dC10aW50LXN0cm9uZyBwb2ludGVyLWZpbmU6Z3JvdXAtZm9jdXMtd2l0aGluL2hhc2g6dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOnBvaW50ZXItZmluZTpncm91cC1mb2N1cy13aXRoaW4vaGFzaDp0ZXh0LXRpbnQtc3Ryb25nIFsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN1YnRsZSBjb250cmFzdC1tb3JlOlsuaGFzaC1yZXZlYWxlZF8mYW1wO106dGV4dC10aW50LXN0cm9uZyBzaXplLTQiPjxwYXRoIGZpbGw9ImN1cnJlbnRDb2xvciIgZD0iTTIyNi45IDI5YzIuOC0xMy01LjUtMjUuNy0xOC40LTI4LjVTMTgyLjcgNiAxODAgMTguOUwxNTYuNiAxMjggNjguNSAxMjhjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuOCAwLTM0LjMgMTYwLTg4IDBjLTEzLjMgMC0yNCAxMC43LTI0IDI0czEwLjcgMjQgMjQgMjRsNzcuNyAwLTIxLjIgOTljLTIuOCAxMyA1LjUgMjUuNyAxOC40IDI4LjVzMjUuNy01LjUgMjguNS0xOC40bDIzLjQtMTA5IDE1NS40IDAtMjEuMiA5OWMtMi44IDEzIDUuNSAyNS43IDE4LjQgMjguNXMyNS43LTUuNSAyOC41LTE4LjRsMjMuNC0xMDkgODggMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuNyAwIDM0LjMtMTYwIDg4LjEgMGMxMy4zIDAgMjQtMTAuNyAyNC0yNHMtMTAuNy0yNC0yNC0yNGwtNzcuOCAwIDIxLjItOTljMi44LTEzLTUuNS0yNS43LTE4LjQtMjguNVMzODcuMiA2IDM4NC40IDE4LjlsLTIzLjQgMTA5LjEtMTU1LjQgMCAyMS4yLTk5ek0xOTUuNCAxNzZsMTU1LjQgMC0zNC4zIDE2MC0xNTUuNCAwIDM0LjMtMTYweiIgLz48L3N2Zz4=)](#closure-captures)

A function can refer to bindings that are declared outside of its own scope. When it does so, it is said to create *captures*. There exists three kind of captures: `let`, `inout` and `sink`.

A `let` capture occurs when a function accesses a binding immutably. For example, in the program below, the closure passed to `map(_:)` creates a `let` capture on `offset`.

An `inout` capture occurs when a function accesses a binding mutably. For example, in the program below, the closure passed to `for_each(_:)` creates an `inout` capture on `sum`.

A `sink` capture occurs when a function acts as a sink for a value at its declaration. Such a capture must be defined explicitly in a capture list. For example, in the program below, `counter` is assigned to a closure that returns integers in incrementing order every time it is called. The closure keeps track of its own state with a `sink` capture.

*Note: The signature of the closure must be annotated with* *`inout`* *because calling it modifies its own state (i.e., the values that it had captured).* *Further, a call to* *`counter`* *must be prefixed by an ampersand to signal mutation.*

[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgMzIwIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2l6ZS0zIj48cGF0aCBmaWxsPSJjdXJyZW50Q29sb3IiIGQ9Ik03LjUgMjM5Yy05LjQgOS40LTkuNCAyNC42IDAgMzMuOWwyMDAgMjAwYzkuNCA5LjQgMjQuNiA5LjQgMzMuOSAwczkuNC0yNC42IDAtMzMuOWwtMTgzLTE4MyAxODMtMTgzYzkuNC05LjQgOS40LTI0LjYgMC0zMy45cy0yNC42LTkuNC0zMy45IDBMNy41IDIzOXoiIC8+PC9zdmc+)](/language-tour/basic-types)

PreviousBasic types

[![](data:image/svg+xml;base64,PHN2ZyB2aWV3Ym94PSIwIDAgMzIwIDUxMiIgZmlsbD0iY3VycmVudENvbG9yIiBzdHlsZT0ib3ZlcmZsb3c6dmlzaWJsZSIgY2xhc3M9ImdiLWljb24gc2l6ZS0zIj48cGF0aCBmaWxsPSJjdXJyZW50Q29sb3IiIGQ9Ik0zMTMuNSAyMzljOS40IDkuNCA5LjQgMjQuNiAwIDMzLjlsLTIwMCAyMDBjLTkuNCA5LjQtMjQuNiA5LjQtMzMuOSAwcy05LjQtMjQuNiAwLTMzLjlsMTgzLTE4My0xODMtMTgzYy05LjQtOS40LTkuNC0yNC42IDAtMzMuOXMyNC42LTkuNCAzMy45IDBsMjAwIDIwMHoiIC8+PC9zdmc+)](/language-tour/subscripts)

NextSubscripts

Last updated 2 years ago
