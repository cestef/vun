# remove result pointers from the language

mlugg 2026-04-22T00:54:26+02:00

In this issue, the terms "result location semantics", "result location", and "result pointer" are **not** intended to include the concept of "result types", which is a core part of Zig's type system whereby type information propagates "down" expression trees to indicate the type an expression "should" have. No change to result types is proposed here.

---

Zig has a feature known as Result Location Semantics (RLS), which in a nutshell guarantees that stores to aggregates do not construct temporaries by forwarding "result pointers" through assignment expressions and providing them to "primitive" expressions to store their result directly to. This feature originally had a few motivations, including:
* Guaranteed copy elision, even through multiple return values, for performance reasons
* Initializing *pinned types*, which were under consideration as a language feature
* Initializing *async frames*, which were also effectively pinned types
* Allowing functions to return self-referential values by directly referencing their result locations

However, RLS comes with several serious issues:
* Unexpected behavior when swapping struct fields: https://github.com/ziglang/zig/issues/12064
* Unexpected `undefined` values when aggregate initialization is short-circuited with a control flow keyword
* Significant language specification and implementation complexity
* (i feel like more things which i forgot?)

Additionally, some of the original use cases don't really hold up:
* Pinned types [have been rejected](https://github.com/ziglang/zig/issues/7769) to maintain language simplicity in favour of userland solutions
* After https://github.com/ziglang/zig/issues/23446, async frames are unlikely to be distinct types, so cannot have "pinned" semantics; and even if they are distinct types, they will typically only be used by `std.Io` implementations, so do not need such strict safety
* Returning self-referential values requires the ability to explicitly refer to a function's result location, which is a problematic language feature so [has been rejected](https://github.com/ziglang/zig/issues/2765)

At this point, the Zig core team broadly considers RLS to be a failed experiment. This issue is to remove the feature from the Zig language, thereby solving the associated footguns and simplifying the compiler implementation.

---

ScottRedig 2026-04-22T03:45:21+02:00

I like this.
Note that this removes the semantic difference between `variable = .{ ...  }` and `variable = T{ ... }`, which further supports [#5038 remove T{} syntax in favor of type coercion](https://github.com/ziglang/zig/issues/5038).
Eg with 0.16.0 behavior showing the difference:
```zig
const print = @import("std").debug.print;
const T = struct { u8, u8 };

pub fn main() void {
    {
        var v: T = .{ 1, 2 };
        v = T{ v[1], v[0] };
        print("{any}\n", .{v}); // .{ 2, 1 }
    }
    {
        var v: T = .{ 1, 2 };
        v = .{ v[1], v[0] };
        print("{any}\n", .{v}); // .{ 2, 2 }
    }
}
```

Also of note, along with [#5973 removing PRO](https://github.com/ziglang/zig/issues/5973#issuecomment-2380332493) this marks the (when implemented) elimination of both 'features' causing issues mentioned in [ATTACK of the KILLER FEATURES (video)](https://www.youtube.com/watch?v=dEIsJPpCZYg).

---

nissarin 2026-04-22T10:30:13+02:00

What's the plan moving forward since the way zig code is written today "guaranteed copy elision" seems like a very important feature, will the compiler be able to "figure it out" even without RLS ?

---

castholm 2026-04-22T13:30:32+02:00

For PRO, [the currently accepted resolution #5973](https://github.com/ziglang/zig/issues/5973#issuecomment-2380332493) is to remove it but to allow an optimizing compiler to promote `fn (ctx: C, key: K) u64` into non-copying `fn (ctx: *const C, key: *const K) u64` if it can statically determine that the function is pure. This seems like the perfect compromise that eliminates several nasty footguns while still providing optimization opportunities, without requiring the user to bend over backwards and use awkward function signatures.

Will the removal of RLS also be resolved in a similar "write the obvious thing and the optimizer will try to elide copies" manner? I'd guess that the LLVM backend already performs generic (N)RVO passes, but will the self-hosted compiler also pursue such optimizations? Unlike the proposed post-PRO optimization, this would require analyzing call sites to ensure that arguments and result locations don't alias (relevant: [noalias on all parameters by default #1108](https://github.com/ziglang/zig/issues/1108)).

---

mlugg 2026-04-22T14:57:40+02:00

@nissarin, @castholm: yes, the idea is that the compiler should be able to optimize this code without RLS, as is the case in most other languages. Note that the self-hosted compiler has **never** supported RLS for function return values, which is typically the only non-trivial case to optimize: the language footguns were too severe to implement it as it existed in the C++ bootstrap compiler.

> will the self-hosted compiler also pursue such optimizations?

In the short term, we intend the self-hosted codegen backends to be used primarily in Debug builds, so optimizations are not relevant. Longer-term, we are planning to implement optimization passes, and when we do so, I don't see any reason RVO wouldn't be on the table.

---

Gota7 2026-05-05T20:07:21+02:00

It looks like this will fix the footgun I encountered with pointers to volatile packed structs in #32174 so I am definitely for this.

---

wzk 2026-05-12T13:29:42+02:00

> In the short term, we intend the self-hosted codegen backends to be used primarily in Debug builds, so optimizations are not relevant.

Debug performance matters though. Since Zig safety approach is to test stuff in debug mode so we can detect early use after free, leaks, ... it's important that debug performance stays good enough to keep a tight feedback loop.
I run a lot of my Zig code from a debugger for that reason. 
(also what's the point of compiling fast if the generated code is python slow ?)

I'm sure some gamedevs could chime here and confirm here that's also a frequent way to work.

I don't know how much this particular proposal will impact the generated code compared to eg the removal of pass-by-reference.
But I think it would be great to keep those elements of Zig success going forward.

---

linus 2026-05-12T14:48:11+02:00

@castholm wrote in https://codeberg.org/ziglang/zig/issues/32009#issuecomment-13536011:

> For PRO, [the currently accepted resolution #5973](https://github.com/ziglang/zig/issues/5973#issuecomment-2380332493) is to remove it

https://github.com/ziglang/zig/issues/5973#issuecomment-4334852241 claims PRO **was** already removed in 0.16. It might have been #25154 but I don't see it mentioned clearly anywhere (only states "partially addresses #5973"), I would appreciate some clarification from the core team about the current state of things.

---

npc1054657282 2026-06-05T01:10:55+02:00

I once felt regret that RLS was about to be removed. But recently I thought of an argument that could serve as a strong reason for removing RLS.
The existence of RLS will attract us to maximize its use, to the point of writing code like the following, because we think RLS can help us reduce copies of `foo` and `bar`:
``` zig
const baz: Baz = .{
    .foo = try .init(allocator);
    .bar = try .init(allocator);
};
```
However, the above code has a memory leak. The correct way is not to use RLS:
``` zig
const foo: Foo = try .init(allocator);
errdefer foo.deinit(allocator);
const bar: Bar = try .init(allocator);
errdefer bar.deinit(allocator);
const baz: Baz = .{ .foo = foo, .bar = bar };
```
I think RLS might write incorrect code in order to attract us to omit copies. At the moment, I can't think of a way to correctly implement this code while using RLS's copy-omission semantics. Therefore, erasing the copy omission of RLS semantically can be justified. We always rely on optimization rather than semantics to omit copies.

---

doerrti 2026-06-05T15:36:46+02:00

How will this change `std`'s APIs? Will they remain unchanged and therefore rely on RVO? It seems that RVO isn't guaranteed in some cases (I didn't do deep research on when exactly it is guaranteed vs. not so maybe that doesn't apply to Zig, i don't know). Before this change is implemented, I'd like to see some kind of data point that confirms that it won't introduce any performance regressions.

---

nmsmith 2026-06-26T03:28:42+02:00

Seems like some of motivation for this proposal would no longer apply if [noalias by default](https://github.com/ziglang/zig/issues/1108) was to land. The noalias issue is still open. Maybe it should be resolved before the current proposal is implemented.

---

zipsegv 2026-08-03T20:34:00+02:00

As a footnote, RLS also causes unintuitive behavior when reassigning unions:

```zig
const Foo = union(enum) { a: u32, b: u32 };

pub fn main() void {
    var foo: Foo = .{ .a = 5 };
    foo = .{ .b = foo.a };
}
```
causes a panic:
```
thread 10307 panic: access of union field 'a' while field 'b' is active
/tmp/test.zig:5:22: 0x11d7ef3 in main (test.zig)
    foo = .{ .b = foo.a };
                     ^
```

---

wzk 2026-08-06T14:47:36+02:00

@zipsegv I feel it's unfair to attribute this problematic behavior (and other in this thread) to RLS. Isn't the issue here that the assignment doesn't copy foo.a on the stack before starting to modify `foo`? Why arent all the loads issued before the stores?

---

nektro 2026-08-06T21:57:55+02:00

> Why arent all the loads issued before the stores?

that is what RLS does.

```
foo = .{ .b = foo.a };
```

in an RLS world is sugar for

```
foo.@tag = .b;
foo.@data = foo.a;
```


---

wzk 2026-08-06T22:15:12+02:00

@nektro wrote in https://codeberg.org/ziglang/zig/issues/32009#issuecomment-20639807:

> that is what RLS does

I understand it's the current behavior, but I feel you could keep 99% of RLS (avoiding memcpy, when building a big struct) if the semantic was a little bit different. 

Writing fields in place is cool, but the interleaving of read and writes is error prone. If you force all the loads to happen before the first store, you will keep a more predictable behavior while editing in place `foo`


---

wzk 2026-09-24T10:24:41+02:00

I'm currently rewriting some part of my code base to avoid stack copies of big structs (mostly due to the removal of PRO IIUC)

And from a correctness perspective RLS is very useful. eg:

```
// compiler checks we set all fields
return .{
    .foo = try .init(allocator),
    .bar = try .init(allocator),
    .baz = try .init(allocator),
};
```

to:

```
out.foo = try .init(allocator);
/// oops we forgot `bar`
out.baz = try .init(allocator);
```

I know there is some ongoing thought around `@useAll` which could be very useful here, since this style becomes required in Zig.

(for errdefer I agree it's better to have the `out.foo` syntax)

---

matklad 2026-09-24T10:40:09+02:00

Maybe I am missing something, but doesn’t the following pattern fix this issue?

```zig
out.* = .{
    .foo = try .init(allocator),
    .bar = try .init(allocator),
    .baz = try .init(allocator),
};
```

a variation:

```
out.foo.initInplace(allocator);
out.* = .{
    .foo = out.foo,
    .bar = try .init(allocator),
    .baz = try .init(allocator),
};
```

---

Khitiara 2026-09-24T19:29:27+02:00

@wzk @matklad those actually arent correct - if the `try .init` fails for bar or baz in your examples, foo is never freed
