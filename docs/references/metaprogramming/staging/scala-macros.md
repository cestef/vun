Macros in Scala 3

# Scala 3 Macros

Language

>  This doc page is specific to Scala 3, and may cover new concepts not available in Scala 2. Unless otherwise stated, all the code examples in this page assume you are using Scala 3.

> Info: JavaScript is currently disabled, code tabs will still work, but preferences will not be remembered.

[Inline methods](/scala3/guides/macros/inline.html) provide us with a elegant technique for metaprogramming by performing some operations at compile time. However, sometimes inlining is not enough and we need more powerful ways to analyze and synthesize programs at compile time. Macros enable us to do exactly this: treat **programs as data** and manipulate them.

## Macros Treat Programs as Values

With a macro, we can treat programs as values, which allows us to analyze and generate them at compile time.

A Scala expression with type `T` is represented by an instance of the type `scala.quoted.Expr[T]`.

We will dig into the details of the type `Expr[T]`, as well as the different ways of analyzing and constructing instances, when talking about [Quoted Code](/scala3/guides/macros/quotes.html) and [Reflection](/scala3/guides/macros/reflection.html). For now, it suffices to know that macros are metaprograms that manipulate expressions of type `Expr[T]`.

The following macro implementation prints the expression of the provided argument at compile-time in the standard output of the compiler process:

``` highlight
import scala.quoted.* // imports Quotes, Expr

def inspectCode(x: Expr[Any])(using Quotes): Expr[Any] =
  println(x.show)
  x
```

After printing the argument expression, we return the original argument as a Scala expression of type `Expr[Any]`.

As foreshadowed in the section on [Inline](/scala3/guides/macros/inline.html), inline methods provide the entry point for macro definitions:

``` highlight
inline def inspect(inline x: Any): Any = ${ inspectCode('x) }
```

All macros are defined with an `inline def`. The implementation of this entry point always has the same shape:

- they only contain a single [splice](/scala3/guides/macros/quotes.html) `${ ... }`
- the splice contains a single call to the method that implements the macro (for example `inspectCode`).
- the call to the macro implementation receives the *quoted* parameters (that is `'x` instead of `x`) and a contextual `Quotes`.

We will dig deeper into these concepts later in this and the following sections.

Calling our `inspect` macro `inspect(sys error "abort")` prints a string representation of the argument expression at compile time:

``` highlight
scala.sys.error("abort")
```

### Macros and Type Parameters

If the macro has type parameters, the implementation will also need to know about them. Just like `scala.quoted.Expr[T]` represents a Scala expression of type `T`, we use `scala.quoted.Type[T]` to represent the Scala type `T`.

``` highlight
inline def logged[T](inline x: T): T = ${ loggedCode('x)  }

def loggedCode[T](x: Expr[T])(using Type[T], Quotes): Expr[T] = ...
```

Both the instance of `Type[T]` and the contextual `Quotes` are automatically provided by the splice in the corresponding inline method (that is, `logged`) and can be used by the macro implementation.

### Defining and Using Macros

A key difference between inlining and macros is the way they are evaluated. Inlining works by rewriting code and performing optimisations based on rules the compiler knows. On the other hand, a macro executes user-written code that generates the code that the macro expands to.

Technically, compiling the inlined code `${ inspectCode('x) }` calls the method `inspectCode` *at compile time* (through Java reflection), and the method `inspectCode` then executes as normal code.

To be able to execute `inspectCode`, we need to compile its source code first. As a technical consequence, we cannot define and use a macro in the **same class/file**. However, it is possible to have the macro definition and its call in the **same project** as long as the implementation of the macro can be compiled first.

> ##### Suspended Files
>
> To allow defining and using macros in the same project, only those calls to macros that have already been compiled are expanded. For all other (unknown) macro calls, the compilation of the file is *suspended*. Suspended files are only compiled after all non suspended files have been successfully compiled. In some cases, you will have *cyclic dependencies* that will block the completion of the compilation. To get more information on which files are suspended you can use the `-Xprint-suspension` compiler flag.

### Example: Statically Evaluating `power` with Macros

Let us recall our definition of `power` from the section on [Inline](/scala3/guides/macros/inline.html) that specialized the computation of `xⁿ` for statically known values of `n`.

``` highlight
inline def power(x: Double, inline n: Int): Double =
  inline if n == 0 then 1.0
  else inline if n % 2 == 1 then x * power(x, n - 1)
  else power(x * x, n / 2)
```

In the remainder of this section, we will define a macro that computes `xⁿ` for a statically known values `x` and `n`. While this is also possible purely with `inline`, implementing it with macros will illustrate a few things.

``` highlight
inline def power(inline x: Double, inline n: Int) =
  ${ powerCode('x, 'n)  }

def powerCode(
  x: Expr[Double],
  n: Expr[Int]
)(using Quotes): Expr[Double] = ...
```

## Simple Expressions

We could implement `powerCode` as follows:

``` highlight
def pow(x: Double, n: Int): Double =
  if n == 0 then 1 else x * pow(x, n - 1)

def powerCode(
  x: Expr[Double],
  n: Expr[Int]
)(using Quotes): Expr[Double] =
  val value: Double = pow(x.valueOrAbort, n.valueOrAbort)
  Expr(value)
```

Here, the `pow` operation is a simple Scala function that computes the value of `xⁿ`. The interesting part is how we create and look into the `Expr`s.

### Creating Expression From Values

Let’s first look at `Expr.apply(value)`. Given a value of type `T`, this call will return an expression containing the code representing the given value (that is, of type `Expr[T]`). The argument value to `Expr` is computed at compile-time, at runtime we only need to instantiate this value.

Creating expressions from values works for all *primitive types*, *tuples* of any arity, `Class`, `Array`, `Seq`, `Set`, `List`, `Map`, `Option`, `Either`, `BigInt`, `BigDecimal`, `StringContext`. Other types can also work if a `ToExpr` is implemented for it, we will [see this later](/scala3/guides/macros/quotes.html).

### Extracting Values from Expressions

The second method we use in the implementation of `powerCode` is `Expr[T].valueOrAbort`, which has an effect opposite to `Expr.apply`. It attempts to extract a value of type `T` from an expression of type `Expr[T]`. This can only succeed, if the expression directly contains the code of a value, otherwise, it will throw an exception that stops the macro expansion and reports that the expression did not correspond to a value.

Instead of `valueOrAbort`, we could also use the `value` operation, which will return an `Option`. This way we can report the error with a custom error message.

#### Reporting Custom Error Messages

The contextual `Quotes` parameter provides a `report` object that we can use to report a custom error message. Within a macro implementation method, you can access the contextual `Quotes` parameter with the `quotes` method (imported with `import scala.quoted.*`), then import the `report` object by `import quotes.reflect.report`.

#### Providing the Custom Error

We will provide the custom error message by calling `errorAndAbort` on the `report` object as follows:

``` highlight
def powerCode(
  x: Expr[Double],
  n: Expr[Int]
)(using Quotes): Expr[Double] =
  import quotes.reflect.report
  (x.value, n.value) match
    case (Some(base), Some(exponent)) =>
      val value: Double = pow(base, exponent)
      Expr(value)
    case (Some(_), _) =>
      report.errorAndAbort("Expected a known value for the exponent, but was " + n.show, n)
    case _ =>
      report.errorAndAbort("Expected a known value for the base, but was " + x.show, x)
```

Alternatively, we can also use the `Expr.unapply` extractor

``` highlight
  ...
  (x, n) match
    case (Expr(base), Expr(exponent)) =>
      val value: Double = pow(base, exponent)
      Expr(value)
    case (Expr(_), _) => ...
    case _ => ...
```

The operations `value`, `valueOrAbort`, and `Expr.unapply` will work for all *primitive types*, *tuples* of any arity, `Option`, `Seq`, `Set`, `Map`, `Either` and `StringContext`. Other types can also work if an `FromExpr` is implemented for it, we will [see this later](/scala3/guides/macros/quotes.html).

### Showing Expressions

In the implementation of `inspectCode`, we have already seen how to convert expressions to the string representation of their *source code* using the `.show` method. This can be useful to perform debugging on macro implementations:

``` highlight
def debugPowerCode(
  x: Expr[Double],
  n: Expr[Int]
)(using Quotes): Expr[Double] =
  println(
    s"powerCode \n" +
    s"  x := ${x.show}\n" +
    s"  n := ${n.show}")
  val code = powerCode(x, n)
  println(s"  code := ${code.show}")
  code
```

### Working with Varargs

Varargs in Scala are represented with `Seq`, hence when we write a macro with a *vararg*, it will be passed as an `Expr[Seq[T]]`. It is possible to recover each individual argument (of type `Expr[T]`) using the `scala.quoted.Varargs` extractor.

``` highlight
import scala.quoted.* // imports `Varargs`, `Quotes`, etc.

inline def sumNow(inline nums: Int*): Int =
  ${ sumCode('nums)  }

def sumCode(nums: Expr[Seq[Int]])(using Quotes): Expr[Int] =
  import quotes.reflect.report
  nums match
    case Varargs(numberExprs) => // numberExprs: Seq[Expr[Int]]
      val numbers: Seq[Int] = numberExprs.map(_.valueOrAbort)
      Expr(numbers.sum)
    case _ => report.errorAndAbort(
      "Expected explicit varargs sequence. " +
      "Notation `args*` is not supported.", nums)
```

The extractor will match a call to `sumNow(1, 2, 3)` and extract a `Seq[Expr[Int]]` containing the code of each parameter. But, if we try to match the argument of the call `sumNow(nums*)`, the extractor will not match.

`Varargs` can also be used as a constructor. `Varargs(Expr(1), Expr(2), Expr(3))` will return an `Expr[Seq[Int]]`. We will see how this can be useful later.

## Complex Expressions

So far, we have only seen how to construct and destruct expressions that correspond to simple values. In order to work with more complex expressions, Scala 3 offers different metaprogramming facilities ranging from

- additional constructors like `Expr.apply`,
- over [quoted pattern matching](/scala3/guides/macros/quotes.html),
- to a full [reflection API](/scala3/guides/macros/reflection.html);

each increasing in complexity and potentially losing safety guarantees. It is generally recommended to prefer simple APIs over more advanced ones. In the remainder of this section, we introduce some more additional constructors and destructors, while subsequent chapters introduce the more advanced APIs.

### Collections

We have seen how to convert a `List[Int]` into an `Expr[List[Int]]` using `Expr.apply`. How about converting a `List[Expr[Int]]` into an `Expr[List[Int]]`? We mentioned that `Varargs.apply` can do this for sequences; likewise, for other collection types, corresponding methods are available:

- `Expr.ofList`: Transform a `List[Expr[T]]` into `Expr[List[T]]`
- `Expr.ofSeq`: Transform a `Seq[Expr[T]]` into `Expr[Seq[T]]` (just like `Varargs`)
- `Expr.ofTupleFromSeq`: Transform a `Seq[Expr[T]]` into `Expr[Tuple]`
- `Expr.ofTuple`: Transform a `(Expr[T1], ..., Expr[Tn])` into `Expr[(T1, ..., Tn)]`

### Simple Blocks

The constructor `Expr.block` provides a simple way to create a block of code `{ stat1; ...; statn; expr }`. Its first arguments is a list with all the statements and the second argument is the expression at the end of the block.

``` highlight
inline def test(inline ignore: Boolean, computation: => Unit): Boolean =
  ${ testCode('ignore, 'computation) }

def testCode(ignore: Expr[Boolean], computation: Expr[Unit])(using Quotes) =
  if ignore.valueOrAbort then Expr(false)
  else Expr.block(List(computation), Expr(true))
```

The `Expr.block` constructor is useful when we want to generate code contanining several side effects. The macro call `test(false, EXPRESSION)` will generate `{ EXPRESSION; true}`, while the call `test(true, EXPRESSION)` will result in `false`.

### Simple Matching

The method `Expr.matches` can be used to check if one expression is equal to another. With this method we could implement an `value` operation for `Expr[Boolean]` as follows.

``` highlight
def value(boolExpr: Expr[Boolean]): Option[Boolean] =
  if boolExpr.matches(Expr(true)) then Some(true)
  else if boolExpr.matches(Expr(false)) then Some(false)
  else None
```

It may also be used to compare two user written expression. Note, that `matches` only performs a limited amount of normalization and while for instance the Scala expression `2` matches the expression `{ 2 }`, this is *not the case* for the expression `{ val x: Int = 2; x }`.

### Arbitrary Expressions

Last but not least, it is possible to create an `Expr[T]` from arbitary Scala code by enclosing it in [quotes](/scala3/guides/macros/quotes.html). For example, `'{ ${expr}; true }` will generate an `Expr[Boolean]` equivalent to `Expr.block(List(expr), Expr(true))`. The subsequent section on [Quoted Code](/scala3/guides/macros/quotes.html) presents quotes in more detail.

[← **previous**](compiletime.html) [**next** →](quotes.html)

### Contributors to this page:

##### Contents

- [Tutorial](/scala3/guides/macros/index.html)
- [Inline](/scala3/guides/macros/inline.html)
- [Scala Compile-time Operations](/scala3/guides/macros/compiletime.html)
- [Scala 3 Macros](/scala3/guides/macros/macros.html)
- [Quoted Code](/scala3/guides/macros/quotes.html)
- [Reflection](/scala3/guides/macros/reflection.html)
- [FAQ](/scala3/guides/macros/faq.html)
- [Best Practices](/scala3/guides/macros/best-practices.html)
- [Other Resources](/scala3/guides/macros/other-resources.html)

------------------------------------------------------------------------

[ Problem with this page?  
     Please help us fix it!](https://github.com/scala/docs.scala-lang/blob/main/_overviews/scala3-macros/tutorial/macros.md)
