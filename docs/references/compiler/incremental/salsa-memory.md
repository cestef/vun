# Memory usage quadrupled after salsa migration

<https://github.com/rust-lang/rust-analyzer/issues/19402>

alexkirsz 2025-03-20T12:44:04Z

**rust-analyzer version**: 2025-03-17

**rustc version**: nightly/2025-01-04

**editor or extension**: Zed

**relevant settings**: N/A

After we updated to the latest release of RA, we've seen its memory usage more than quadruple when running on our codebase. Before the update, RA was routinely taking between 5 to 6GB of memory. After the update, it balloons up to 22GB a few seconds after starting, and can increase up to 30GB during use.

I've bisected the regression to https://github.com/rust-lang/rust-analyzer/commit/74620e6, which is the port to the new salsa.

I'm currently trying to capture allocations profiles using Instruments on macOS, but the instrumentation makes RA extremely slow, so it takes quite a while to run. I'll try on a subset of our codebase next, which hopefully should reproduce the issue while being faster to run, and update this issue with the results.

Particularities of our setup:

1. We have about 700 crates, 2800 counting external dependencies.
2. We use Bazel instead of cargo, which means we use `rust-project.json` (or more specifically, [the ongoing discover config integration](https://github.com/bazelbuild/rules_rust/pull/3073)).

---

alexkirsz 2025-03-20T15:33:35Z

Here are two Instruments.app Allocations traces. I had to run rust-analyzer on a small subset of our workspace (~80 crates), as otherwise running the instrumentation would have taken hours on the full workspace. Even on ~10% of the workspace, Instruments is no longer able to save/reopen the .trace files it creates.

On this subset, I'm only seeing a 2x increase in memory usage, but hopefully it is representative of the underlying issue(s).

The first trace is running on 394374e, the second is running on 74620e6 (after #18964). The traces are very large (10GiB), so I've gzipped them.

1. [705,48MiB ](https://storage.googleapis.com/amo-alex-traces/trace_before.trace.tar.gz)
2. [1,60GiB](https://storage.googleapis.com/amo-alex-traces/trace_after.trace.tar.gz)

Please let me know if there are better ways to run memory/heap allocations profiling for RA!

---

davidbarsky 2025-03-20T16:25:35Z

Oh boy, 30 gigabytes is suboptimal. As least temporarily, can you try disabling cache priming in your editor? Here's the configuration you'll need in Zed:

```json
 "lsp": {
    "rust-analyzer": {
      "initialization_options": {
        "cachePriming": {
          "enable": false
        }
      }
    }
  },
```

Slightly longer term, I _think_ I have a sense as to what's going on: I think cache priming ("Indexing") doesn't increment the revision counter, so we never actually garbage collect _anything_. cc: @Veykril—how wrong am I about this theory?

---

alexkirsz 2025-03-20T16:43:49Z

@davidbarsky After disabling cache priming, rust-analyzer now only takes 5.5GiB of memory. It's also much faster to start, so I think I'm never going back 😄

Thanks for the quick reply. I had built an older version of RA for the team, but this completely unblocks us for now.

**EDIT:** Okay, the "much faster to start" part might have been placebo, I'll need to run more comparisons, but it does feel noticeably faster than before the salsa migration.

---

davidbarsky 2025-03-20T21:46:16Z

> [@davidbarsky](https://github.com/davidbarsky) After disabling cache priming, rust-analyzer now only takes 5.5GiB of memory. It's also much faster to start, so I think I'm never going back 😄

I'm guessing you already discovered this, but disabling cache priming means that startup is faster, but that faster startup time is paid for by certain operations like symbol search or go-to-def being slightly slower because, well—the indexes they rely on haven't been *built* yet. We might need to fix stick a [Database::trigger_lru_eviction](https://docs.rs/salsa/0.19.0/salsa/trait.Database.html#method.trigger_lru_eviction)

> **EDIT:** Okay, the "much faster to start" part might have been placebo, I'll need to run more comparisons, but it does feel noticeably faster than before the salsa migration.

See the disclaimer above, but I've noticed that new Salsa *is* slightly faster on some projects even with cache priming. My suggestion to disable cache priming was intended to buy us time in order to run garbage collection during cache priming.

---

Rudxain 2025-04-16T21:12:18Z

IDK if this helps narrow down the problem, but I haven't observed RA using excessive mem on the [UADNG](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation) repo

---

cormacrelf 2025-06-04T18:29:09Z


> I had to run rust-analyzer on a small subset of our workspace (~80 crates), as otherwise running the instrumentation would have taken hours on the full workspace.

> Please let me know if there are better ways to run memory/heap allocations profiling for RA!

In situations like these, https://docs.rs/allocative can be useful. It does something like a GC trace on all active allocations given a root object (eg your god object, whatever owns all the data), and produces flame graph of memory use, nesting indicating ownership. It's a bit of work to derive it everywhere but it seems worthwhile in the circumstances.

Because it has no overhead aside from binary size, you can compile it into release builds and take samples from the wild where and when memory issues are occurring. Just have a button to save a flame graph of current memory use. You're supposed to be able to open the traces in eg Firefox Profiler and do analysis usually reserved for stack tracing profilers, like "invert call tree".

---

MichaelMcDonnell 2025-06-18T15:06:21Z

I had the same problem with [Servo](https://github.com/servo/servo) on NixOS but upgrading from Rust 1.85 to 1.86 (or 1.87) fixed it for me. I don't have to disable cache priming. @alexkirsz are you still able to reproduce it with newer versions of Rust?

---

Rudxain 2025-07-08T11:42:01Z

> I haven't observed RA using excessive mem on the [UADNG](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation) repo

Now it's **3.2GB RSS** when it was previously 0.4GB (IIRC), both stats were with **cache-priming disabled**. It seems the memory-usage increased on Universal-Debloater-Alliance/universal-android-debloater-next-generation#949. Can repro on 1.90.0-nightly (5adb489 2025-07-05)

---

Veykril 2025-07-08T12:36:38Z

Do you have a version (ideally as recent as possible) that does not show excessive memory usage on that repo?

---

Rudxain 2025-07-09T07:21:57Z

> Do you have a version (ideally as recent as possible) that does not show excessive memory usage on that repo?

Not found, yet.

But on [this commit](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation/commit/4ffdecd12321ea0987348e4aebb890f0465612a8), I've opened a file with Helix:
```sh
hx src/core/adb.rs
```
It's 2.9GB on startup (after waiting a few minutes).

On the branch that upgrades deps, the RSS stabilizes at 3.0 instead. Keep in mind that the RAM has already been exhausted, so the paging to swap will decrease the RSS. I've noticed that if I wait long enough (not touching the editor at all), RSS gradually goes down (not sure if it's because of swap, or actual freed mem)

In all cases so far, the CPU maxes while the memory is filling up (and most LSP features are unusable until it stabilizes)

---

flodiebold 2025-07-09T07:47:18Z

3G on a codebase that includes Iced is not excessive, it's pretty much within expectations.

---

Rudxain 2025-07-09T07:57:17Z

> 3G on a codebase that includes Iced is not excessive, it's pretty much within expectations.

`~/.config/helix/languages.toml`:
```toml
# ...
[language-server.rust-analyzer.config.cachePriming]
enable = false
# ...
```
It never went above 1GB on startup. And even after normal usage, I remember it was below 2GB

---

cormacrelf 2025-07-09T09:56:55Z

https://gist.github.com/cormacrelf/4715d7b09146107045427ed508153f98

Linux-only. To run, using facebook/buck2 as the test repo:

```sh
# (download gist outside rust-analyzer folder)
cd rust-analyzer
# make sure rustup is up to date enough to build all of it
git checkout origin/release
rustup update stable

# establish baseline
git checkout 2024-09-23
../ra-bisect.sh ~/code/oss/buck2 ~/code/oss/buck2/app/buck2/bin/buck2.rs
# exit when it stabilises (press r to refresh a few times after the 60 seconds are up)

git checkout origin/release
../ra-bisect.sh ~/code/oss/buck2 ~/code/oss/buck2/app/buck2/bin/buck2.rs
# same

# start bisecting
git bisect start origin/release 2024-09-23
git bisect run ../ra-bisect.sh ~/code/oss/buck2 ~/code/oss/buck2/app/buck2/bin/buck2.rs
```

It will prompt you to evaluate the memory usage yourself. My bisect is still going but baselines are:

- 2024-09-23: 2200 MB
- origin/release today: 4385 MB

I'm still bisecting.

---

cormacrelf 2025-07-09T10:43:31Z

Yep, it's basically the salsa migration but memory usage has come down from that peak.

74620e64ec24861a821ebb9e519461042905668f

```
Waiting 60 seconds...
Current memory usage: 2050 MB
Current memory usage: 3309 MB
Current memory usage: 3722 MB
Current memory usage: 4389 MB
Current memory usage: 5306 MB
Current memory usage: 5306 MB

Current memory usage: 5306 MB

Does this commit exhibit high memory usage? (y/n/r) y
Marking commit as BAD.
74620e64ec24861a821ebb9e519461042905668f is the first bad commit
commit 74620e64ec24861a821ebb9e519461042905668f
Author: David Barsky <me@davidbarsky.com>
Date:   Tue Nov 5 12:24:41 2024 -0500

    internal: port rust-analyzer to new Salsa
```

---

Logarithmus 2025-07-24T17:48:29Z

> Slightly longer term, I _think_ I have a sense as to what's going on: I think cache priming ("Indexing") doesn't increment the revision counter, so we never actually garbage collect _anything_. cc: [@Veykril](https://github.com/Veykril)—how wrong am I about this theory?

@davidbarsky @Veykril Excuse me, could you please elaborate on this? What is revision counter? Could you point at the place in the code where to look? I'd like to investigate this issue deeply and fix it, because memory usage with new Salsa essentially *doubled*  on my project, and I don't want to be stuck on 2025-03-10 version forever...

Or should I write to Zulip instead?

---

ChayimFriedman2 2025-07-24T18:04:01Z

@Logarithmus The comment you are referring to was already fixed.

And yes, it's easier to communicate ideas in Zulip.

---

RalfJung 2025-10-19T11:53:24Z

I'm seeing 8.4GB RAM usage on the rustc workspace... and I often have more than one vscode window open. Together with the other stuff that runs in the background and the RAM needed for the rustc build instead, it has become noticeably harder to do rustc development on my 32GB RAM laptop. I now fairly often see vscode windows killed by the OOM killer which hasn't been a problem in the past.

---

Veykril 2025-10-19T15:45:29Z

Afaik we are soon getting rid of chalk entirely which will likely have some reduction on memory again. I am hopeful that once the trait solver migration is done we can take a look at memory with a fresh pair of eyes again

---

Rudxain 2025-10-19T16:28:55Z

> Edit: I've wrote this before `chalk` was mentioned

I've been considering, for a long time, that RA needs a rewrite from the ground up. **feature-freeze is a must** at this point, both to reduce scope-creep, and to ease the rewrite process. I'm willing to help with the rewrite, as I'm interested in building a lang-server (especially parsers that don't rely on regexes).

## Reasons why I believe rewriting is the way to go

### `clangd`
The fact that `clangd` can insta-start on a big code-base such as (_the_) [`git`](https://github.com/git/git/) (granted, because of its fail-fast approach, it stops analyzing after a few errors, if the user hasn't configured it), while RA struggles on a low-sized one with cache-priming disabled, should be enough evidence that there's **a lot of room for improvement**.

I'm sorry if that's an unfair comparison, but the difference I've seen is about **80x speed**. If RA had a full borrow-ck implementation that'd be acceptable, but that is not the case (or is it?).

### `grep` vs find-refs

`LANG=C.UTF-8 grep -ri` (not [`rg`](https://github.com/BurntSushi/ripgrep/blob/d47663b1b4548e4fa02d6e4b575718d0f5f5e7d6/README.md#quick-examples-comparing-tools)!) is orders-of-magnitude faster than RA's find-all-references (granted, cache-priming is disabled), and sometimes even when `grep` searches `.git/` (while RA ignores it).

Granted, searching slices and tree-walking are different operations. But the point is that I'm more productive using **any tool** for global-search other than RA's symbol-aware engine, and they're "more reliable" despite the false-positives and false-negatives. In other words, I'd rather get "bad results" from a generic search tool than await RA to give me "100% verified" results.

### memory
The main topic of this issue. (I'm sorry for being off-topic 😅)

I've been forced to use RA only for tiny code-bases (my current laptop is a "potato"). But RA is designed to be helpful for mid/big projects (am I wrong?). What does that mean? That RA's purpose is unfulfilled for many users, making it "effectively useless", despite its awesome potential. This is depressing and frustrating, at least for me

---

ShoyuVanilla 2025-10-19T17:36:57Z

> I've been considering, for a long time, that RA needs a rewrite from the ground up. **feature-freeze is a must** at this point, both to reduce scope-creep, and to ease the rewrite process.

I agree with that current rust-analyzer has lots of rooms for improvements and it has many false positives but comparision with `clangd` or mere grepping is unfair, especially when cache-priming is disabled (rust-analyzer runs in an incremental way and `salsa` is for that)

Unlike C, which is a relatively simple language, Rust has many complex features other than borrowck like trait system and they needs many heavy liftings. For example, if your code calls a method to a certain variable, the compiler/language server has to look-up not only its inherent impls but also traits it might implement. And things are more complicated when the trait in question is a (blanket impl) with bounds.

And for reference/symbol search, yeah, it's inevitably slower than grep when there's no cache available yet, because it has to infer types for usage sites because we are trying to be as accurate as possible. But there are still bugs - mostly due to type inference failures - as you mentioned. However, things are getting better as we have been fixing many type errors and the next-trait solver migration improves it by a lot. I personally use both grep(with telescope) and rust-analyzer's symbol search and they feel like complementary tools.

> I'm willing to help with the rewrite, as I'm interested in building a lang-server (especially parsers that don't rely on regexes).

Do you have some opinions on what designs of rust-analyzer are problematic and what would be alternatives for them? Rewrite from the ground up with (possibly months of) feature freeze is a big thing, so that would require sufficient discussions, design choices and experiments.

---

Rudxain 2025-10-19T19:52:53Z

Should we move to Zulip, Discourse forum, or a separate issue? I'm asking just-in-case, to avoid being off-topic

---

ChayimFriedman2 2025-10-19T19:59:37Z

@Rudxain You can open a new topic in Zulip if you want.

---

flodiebold 2025-10-19T21:39:11Z

Or a GitHub discussion.

---

Rudxain 2025-10-20T12:00:39Z

#20874

---

Veykril 2025-12-30T20:39:35Z

I think we can close this, after @ChayimFriedman2's massive efforts we are back to pre-salsa migration memory levels (in fact, we are slightly better, especially considering the fact that the r-a codebase has grown since)

