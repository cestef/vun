# Use annotate-snippet as default emitter on stable

<https://github.com/rust-lang/rust/pull/150032>

Kivooeo 2025-12-15T19:22:35Z

<!-- homu-ignore:start -->
<!--
If this PR is related to an unstable feature or an otherwise tracked effort,
please link to the relevant tracking issue here. If you don't know of a related
tracking issue or there are none, feel free to ignore this.

This PR will get automatically assigned to a reviewer. In case you would like
a specific user to review your work, you can assign it to them by using

    r? <reviewer name>
-->
<!-- homu-ignore:end -->

This is implementation of https://github.com/rust-lang/rust/issues/149932

Now, after MCP was accepted, we can use annotate-snippet as default emitter for errors, that means that we not longer need of previous emitter, so this PR removed previous emitter and makes annotate-snippet new default one both on stable and nightly

(this PR does not remove a code of previous emitter it just removes a `Default` option of `HumanReadableErrorType` enum, and keeping only `HumanReadableErrorType::AnnotateSnippet` as it now uses by default)

---

rust-log-analyzer 2025-12-15T20:06:15Z


The job **`x86_64-gnu-gcc`** failed! Check out the build log: [(web)](https://github.com/rust-lang/rust/actions/runs/20244684736/job/58121610492) [(plain enhanced)](https://triage.rust-lang.org/gha-logs/rust-lang/rust/58121610492) [(plain)](https://github.com/rust-lang/rust/commit/e4ad7603699d59901eb4e7806c2edaaf96697caa/checks/58121610492/logs)

<details><summary><i>Click to see the possible cause of the failure (guessed by this bot)</i></summary>

```plain
---- [run-make] tests/run-make/const-trait-stable-toolchain stdout ----

error: rmake recipe failed to complete
status: exit status: 101
command: cd "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/const-trait-stable-toolchain/rmake_out" && env -u RUSTFLAGS -u __RUSTC_DEBUG_ASSERTIONS_ENABLED -u __STD_DEBUG_ASSERTIONS_ENABLED AR="ar" BUILD_ROOT="/checkout/obj/build/x86_64-unknown-linux-gnu" CC="cc" CC_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" CXX="c++" CXX_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" HOST_RUSTC_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib" LD_LIBRARY_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/bootstrap-tools/x86_64-unknown-linux-gnu/release/deps:/checkout/obj/build/x86_64-unknown-linux-gnu/stage0/lib/rustlib/x86_64-unknown-linux-gnu/lib" LD_LIB_PATH_ENVVAR="LD_LIBRARY_PATH" LLVM_BIN_DIR="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin" LLVM_COMPONENTS="aarch64 aarch64asmparser aarch64codegen aarch64desc aarch64disassembler aarch64info aarch64utils aggressiveinstcombine all all-targets amdgpu amdgpuasmparser amdgpucodegen amdgpudesc amdgpudisassembler amdgpuinfo amdgputargetmca amdgpuutils analysis arm armasmparser armcodegen armdesc armdisassembler arminfo armutils asmparser asmprinter avr avrasmparser avrcodegen avrdesc avrdisassembler avrinfo binaryformat bitreader bitstreamreader bitwriter bpf bpfasmparser bpfcodegen bpfdesc bpfdisassembler bpfinfo cfguard cgdata codegen codegentypes core coroutines coverage csky cskyasmparser cskycodegen cskydesc cskydisassembler cskyinfo debuginfobtf debuginfocodeview debuginfodwarf debuginfodwarflowlevel debuginfogsym debuginfologicalview debuginfomsf debuginfopdb demangle dlltooldriver dwarfcfichecker dwarflinker dwarflinkerclassic dwarflinkerparallel dwp engine executionengine extensions filecheck frontendatomic frontenddirective frontenddriver frontendhlsl frontendoffloading frontendopenacc frontendopenmp fuzzercli fuzzmutate globalisel hexagon hexagonasmparser hexagoncodegen hexagondesc hexagondisassembler hexagoninfo hipstdpar instcombine instrumentation interfacestub interpreter ipo irprinter irreader jitlink libdriver lineeditor linker loongarch loongarchasmparser loongarchcodegen loongarchdesc loongarchdisassembler loongarchinfo lto m68k m68kasmparser m68kcodegen m68kdesc m68kdisassembler m68kinfo mc mca mcdisassembler mcjit mcparser mips mipsasmparser mipscodegen mipsdesc mipsdisassembler mipsinfo mirparser msp430 msp430asmparser msp430codegen msp430desc msp430disassembler msp430info native nativecodegen nvptx nvptxcodegen nvptxdesc nvptxinfo objcarcopts objcopy object objectyaml option orcdebugging orcjit orcshared orctargetprocess passes powerpc powerpcasmparser powerpccodegen powerpcdesc powerpcdisassembler powerpcinfo profiledata remarks riscv riscvasmparser riscvcodegen riscvdesc riscvdisassembler riscvinfo riscvtargetmca runtimedyld sandboxir scalaropts selectiondag sparc sparcasmparser sparccodegen sparcdesc sparcdisassembler sparcinfo support symbolize systemz systemzasmparser systemzcodegen systemzdesc systemzdisassembler systemzinfo tablegen target targetparser telemetry textapi textapibinaryreader transformutils vectorize webassembly webassemblyasmparser webassemblycodegen webassemblydesc webassemblydisassembler webassemblyinfo webassemblyutils windowsdriver windowsmanifest x86 x86asmparser x86codegen x86desc x86disassembler x86info x86targetmca xray xtensa xtensaasmparser xtensacodegen xtensadesc xtensadisassembler xtensainfo" LLVM_FILECHECK="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin/FileCheck" PYTHON="/usr/bin/python3" RUSTC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustc" RUSTDOC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustdoc" SOURCE_ROOT="/checkout" TARGET="x86_64-unknown-linux-gnu" TARGET_EXE_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/x86_64-unknown-linux-gnu/lib" "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/const-trait-stable-toolchain/rmake"
stdout: none
--- stderr -------------------------------

thread 'main' (38747) panicked at /checkout/tests/run-make/const-trait-stable-toolchain/rmake.rs:23:10:
test failed: `const-super-trait-stable-enabled.stderr` is different from `(rustc)`

--- const-super-trait-stable-enabled.stderr
+++ (rustc)
@@ -1,62 +1,2 @@
-error: `[const]` is not allowed here
- --> const-super-trait.rs:7:12
-  |
-7 | trait Bar: [const] Foo {}
-  |            ^^^^^^^
-  |
-note: this trait is not `const`, so it cannot have `[const]` trait bounds
- --> const-super-trait.rs:7:1
-  |
-7 | trait Bar: [const] Foo {}
-  | ^^^^^^^^^^^^^^^^^^^^^^^^^
-
-error[E0554]: `#![feature]` may not be used on the NIGHTLY release channel
- --> const-super-trait.rs:1:30
-  |
-1 | #![cfg_attr(feature_enabled, feature(const_trait_impl))]
-  |                              ^^^^^^^^^^^^^^^^^^^^^^^^^
-
-error: `[const]` can only be applied to `const` traits
- --> const-super-trait.rs:7:12
-  |
-7 | trait Bar: [const] Foo {}
-  |            ^^^^^^^ can't be applied to `Foo`
-  |
-note: `Foo` can't be used with `[const]` because it isn't `const`
- --> const-super-trait.rs:3:1
-  |
-3 | trait Foo {
-  | ^^^^^^^^^
-
-error: `[const]` can only be applied to `const` traits
- --> const-super-trait.rs:9:17
-  |
-9 | const fn foo<T: [const] Bar>(x: &T) {
-  |                 ^^^^^^^ can't be applied to `Bar`
-  |
-note: `Bar` can't be used with `[const]` because it isn't `const`
- --> const-super-trait.rs:7:1
-  |
-7 | trait Bar: [const] Foo {}
-  | ^^^^^^^^^^^^^^^^^^^^^^
+error: `--error-format=human-annotate-rs` is unstable
 
-error[E0015]: cannot call non-const method `<T as Foo>::a` in constant functions
-  --> const-super-trait.rs:10:7
-   |
-10 |     x.a();
-   |       ^^^
-   |
---
   0: __rustc::rust_begin_unwind
             at /rustc/3b4dd9bf1410f8da6329baa36ce5e37673cbbd1f/library/std/src/panicking.rs:698:5
   1: core::panicking::panic_fmt
             at /rustc/3b4dd9bf1410f8da6329baa36ce5e37673cbbd1f/library/core/src/panicking.rs:80:14
   2: <run_make_support::diff::Diff>::run
   3: rmake::main
   4: core::ops::function::FnOnce::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
------------------------------------------

---- [run-make] tests/run-make/const-trait-stable-toolchain stdout end ----
---- [run-make] tests/run-make/missing-unstable-trait-bound stdout ----

error: rmake recipe failed to complete
status: exit status: 101
command: cd "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/missing-unstable-trait-bound/rmake_out" && env -u RUSTFLAGS -u __RUSTC_DEBUG_ASSERTIONS_ENABLED -u __STD_DEBUG_ASSERTIONS_ENABLED AR="ar" BUILD_ROOT="/checkout/obj/build/x86_64-unknown-linux-gnu" CC="cc" CC_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" CXX="c++" CXX_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" HOST_RUSTC_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib" LD_LIBRARY_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/bootstrap-tools/x86_64-unknown-linux-gnu/release/deps:/checkout/obj/build/x86_64-unknown-linux-gnu/stage0/lib/rustlib/x86_64-unknown-linux-gnu/lib" LD_LIB_PATH_ENVVAR="LD_LIBRARY_PATH" LLVM_BIN_DIR="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin" LLVM_COMPONENTS="aarch64 aarch64asmparser aarch64codegen aarch64desc aarch64disassembler aarch64info aarch64utils aggressiveinstcombine all all-targets amdgpu amdgpuasmparser amdgpucodegen amdgpudesc amdgpudisassembler amdgpuinfo amdgputargetmca amdgpuutils analysis arm armasmparser armcodegen armdesc armdisassembler arminfo armutils asmparser asmprinter avr avrasmparser avrcodegen avrdesc avrdisassembler avrinfo binaryformat bitreader bitstreamreader bitwriter bpf bpfasmparser bpfcodegen bpfdesc bpfdisassembler bpfinfo cfguard cgdata codegen codegentypes core coroutines coverage csky cskyasmparser cskycodegen cskydesc cskydisassembler cskyinfo debuginfobtf debuginfocodeview debuginfodwarf debuginfodwarflowlevel debuginfogsym debuginfologicalview debuginfomsf debuginfopdb demangle dlltooldriver dwarfcfichecker dwarflinker dwarflinkerclassic dwarflinkerparallel dwp engine executionengine extensions filecheck frontendatomic frontenddirective frontenddriver frontendhlsl frontendoffloading frontendopenacc frontendopenmp fuzzercli fuzzmutate globalisel hexagon hexagonasmparser hexagoncodegen hexagondesc hexagondisassembler hexagoninfo hipstdpar instcombine instrumentation interfacestub interpreter ipo irprinter irreader jitlink libdriver lineeditor linker loongarch loongarchasmparser loongarchcodegen loongarchdesc loongarchdisassembler loongarchinfo lto m68k m68kasmparser m68kcodegen m68kdesc m68kdisassembler m68kinfo mc mca mcdisassembler mcjit mcparser mips mipsasmparser mipscodegen mipsdesc mipsdisassembler mipsinfo mirparser msp430 msp430asmparser msp430codegen msp430desc msp430disassembler msp430info native nativecodegen nvptx nvptxcodegen nvptxdesc nvptxinfo objcarcopts objcopy object objectyaml option orcdebugging orcjit orcshared orctargetprocess passes powerpc powerpcasmparser powerpccodegen powerpcdesc powerpcdisassembler powerpcinfo profiledata remarks riscv riscvasmparser riscvcodegen riscvdesc riscvdisassembler riscvinfo riscvtargetmca runtimedyld sandboxir scalaropts selectiondag sparc sparcasmparser sparccodegen sparcdesc sparcdisassembler sparcinfo support symbolize systemz systemzasmparser systemzcodegen systemzdesc systemzdisassembler systemzinfo tablegen target targetparser telemetry textapi textapibinaryreader transformutils vectorize webassembly webassemblyasmparser webassemblycodegen webassemblydesc webassemblydisassembler webassemblyinfo webassemblyutils windowsdriver windowsmanifest x86 x86asmparser x86codegen x86desc x86disassembler x86info x86targetmca xray xtensa xtensaasmparser xtensacodegen xtensadesc xtensadisassembler xtensainfo" LLVM_FILECHECK="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin/FileCheck" PYTHON="/usr/bin/python3" RUSTC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustc" RUSTDOC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustdoc" SOURCE_ROOT="/checkout" TARGET="x86_64-unknown-linux-gnu" TARGET_EXE_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/x86_64-unknown-linux-gnu/lib" "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/missing-unstable-trait-bound/rmake"
stdout: none
--- stderr -------------------------------
assert_contains:
=== HAYSTACK ===
error: `--error-format=human-annotate-rs` is unstable


=== NEEDLE ===

  = note: required for `std::ops::Range<T>` to implement `Iterator`
---
---- [run-make] tests/run-make/print-request-help-stable-unstable stdout ----

error: rmake recipe failed to complete
status: exit status: 101
command: cd "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/print-request-help-stable-unstable/rmake_out" && env -u RUSTFLAGS -u __RUSTC_DEBUG_ASSERTIONS_ENABLED -u __STD_DEBUG_ASSERTIONS_ENABLED AR="ar" BUILD_ROOT="/checkout/obj/build/x86_64-unknown-linux-gnu" CC="cc" CC_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" CXX="c++" CXX_DEFAULT_FLAGS="-ffunction-sections -fdata-sections -fPIC -m64" HOST_RUSTC_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib" LD_LIBRARY_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/bootstrap-tools/x86_64-unknown-linux-gnu/release/deps:/checkout/obj/build/x86_64-unknown-linux-gnu/stage0/lib/rustlib/x86_64-unknown-linux-gnu/lib" LD_LIB_PATH_ENVVAR="LD_LIBRARY_PATH" LLVM_BIN_DIR="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin" LLVM_COMPONENTS="aarch64 aarch64asmparser aarch64codegen aarch64desc aarch64disassembler aarch64info aarch64utils aggressiveinstcombine all all-targets amdgpu amdgpuasmparser amdgpucodegen amdgpudesc amdgpudisassembler amdgpuinfo amdgputargetmca amdgpuutils analysis arm armasmparser armcodegen armdesc armdisassembler arminfo armutils asmparser asmprinter avr avrasmparser avrcodegen avrdesc avrdisassembler avrinfo binaryformat bitreader bitstreamreader bitwriter bpf bpfasmparser bpfcodegen bpfdesc bpfdisassembler bpfinfo cfguard cgdata codegen codegentypes core coroutines coverage csky cskyasmparser cskycodegen cskydesc cskydisassembler cskyinfo debuginfobtf debuginfocodeview debuginfodwarf debuginfodwarflowlevel debuginfogsym debuginfologicalview debuginfomsf debuginfopdb demangle dlltooldriver dwarfcfichecker dwarflinker dwarflinkerclassic dwarflinkerparallel dwp engine executionengine extensions filecheck frontendatomic frontenddirective frontenddriver frontendhlsl frontendoffloading frontendopenacc frontendopenmp fuzzercli fuzzmutate globalisel hexagon hexagonasmparser hexagoncodegen hexagondesc hexagondisassembler hexagoninfo hipstdpar instcombine instrumentation interfacestub interpreter ipo irprinter irreader jitlink libdriver lineeditor linker loongarch loongarchasmparser loongarchcodegen loongarchdesc loongarchdisassembler loongarchinfo lto m68k m68kasmparser m68kcodegen m68kdesc m68kdisassembler m68kinfo mc mca mcdisassembler mcjit mcparser mips mipsasmparser mipscodegen mipsdesc mipsdisassembler mipsinfo mirparser msp430 msp430asmparser msp430codegen msp430desc msp430disassembler msp430info native nativecodegen nvptx nvptxcodegen nvptxdesc nvptxinfo objcarcopts objcopy object objectyaml option orcdebugging orcjit orcshared orctargetprocess passes powerpc powerpcasmparser powerpccodegen powerpcdesc powerpcdisassembler powerpcinfo profiledata remarks riscv riscvasmparser riscvcodegen riscvdesc riscvdisassembler riscvinfo riscvtargetmca runtimedyld sandboxir scalaropts selectiondag sparc sparcasmparser sparccodegen sparcdesc sparcdisassembler sparcinfo support symbolize systemz systemzasmparser systemzcodegen systemzdesc systemzdisassembler systemzinfo tablegen target targetparser telemetry textapi textapibinaryreader transformutils vectorize webassembly webassemblyasmparser webassemblycodegen webassemblydesc webassemblydisassembler webassemblyinfo webassemblyutils windowsdriver windowsmanifest x86 x86asmparser x86codegen x86desc x86disassembler x86info x86targetmca xray xtensa xtensaasmparser xtensacodegen xtensadesc xtensadisassembler xtensainfo" LLVM_FILECHECK="/checkout/obj/build/x86_64-unknown-linux-gnu/ci-llvm/bin/FileCheck" PYTHON="/usr/bin/python3" RUSTC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustc" RUSTDOC="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/bin/rustdoc" SOURCE_ROOT="/checkout" TARGET="x86_64-unknown-linux-gnu" TARGET_EXE_DYLIB_PATH="/checkout/obj/build/x86_64-unknown-linux-gnu/stage2/lib/rustlib/x86_64-unknown-linux-gnu/lib" "/checkout/obj/build/x86_64-unknown-linux-gnu/test/run-make/print-request-help-stable-unstable/rmake"
stdout: none
--- stderr -------------------------------

thread 'main' (50418) panicked at /checkout/tests/run-make/print-request-help-stable-unstable/rmake.rs:17:10:
test failed: `stable-invalid-print-request-help.err` is different from `stable_invalid_print_request_help`

--- stable-invalid-print-request-help.err
+++ stable_invalid_print_request_help
@@ -1,5 +1,2 @@
-error: unknown print request: `xxx`
-  |
-  = help: valid print requests are: `calling-conventions`, `cfg`, `code-models`, `crate-name`, `deployment-target`, `file-names`, `host-tuple`, `link-args`, `native-static-libs`, `relocation-models`, `split-debuginfo`, `stack-protector-strategies`, `sysroot`, `target-cpus`, `target-features`, `target-libdir`, `target-list`, `tls-models`
-  = help: for more information, see the rustc book: https://doc.rust-lang.org/rustc/command-line-arguments.html#--print-print-compiler-information
+error: `--error-format=human-annotate-rs` is unstable
 

stack backtrace:
   0: __rustc::rust_begin_unwind
             at /rustc/3b4dd9bf1410f8da6329baa36ce5e37673cbbd1f/library/std/src/panicking.rs:698:5
   1: core::panicking::panic_fmt
             at /rustc/3b4dd9bf1410f8da6329baa36ce5e37673cbbd1f/library/core/src/panicking.rs:80:14
   2: <run_make_support::diff::Diff>::run
   3: rmake::main
   4: core::ops::function::FnOnce::call_once
note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.
------------------------------------------

---- [run-make] tests/run-make/print-request-help-stable-unstable stdout end ----
```

</details>

For more information how to resolve CI failures of this job, visit this [link](https://rustc-dev-guide.rust-lang.org/tests/codegen-backend-tests/cg_gcc.html).

---

rust-log-analyzer 2025-12-15T21:36:16Z


The job **`x86_64-gnu-tools`** failed! Check out the build log: [(web)](https://github.com/rust-lang/rust/actions/runs/20246630570/job/58128300636) [(plain enhanced)](https://triage.rust-lang.org/gha-logs/rust-lang/rust/58128300636) [(plain)](https://github.com/rust-lang/rust/commit/0298975250ebde2740912ab0658b5b55dfbf8a3d/checks/58128300636/logs)

<details><summary><i>Click to see the possible cause of the failure (guessed by this bot)</i></summary>

```plain
tests/ui/should_impl_trait/method_list_2.rs (revision `edition2021`) ... ok
tests/ui/size_of_in_element_count/functions.rs ... ok

FAILED TEST: tests/ui/unit_arg.rs
command: CLIPPY_CONF_DIR="tests" RUSTC_ICE="0" "/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/release/clippy-driver" "--error-format=json" "--emit=metadata" "-Aunused" "-Ainternal_features" "-Zui-testing" "-Zdeduplicate-diagnostics=no" "-Dwarnings" "-Ldependency=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/release/deps" "--sysroot=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2" "--out-dir" "/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/ui_test/0/tests/ui" "tests/ui/unit_arg.rs" "--extern" "proc_macros=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/ui_test/0/tests/ui/auxiliary/libproc_macros.so" "-L" "/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/ui_test/0/tests/ui/auxiliary" "--extern" "futures=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libfutures-56bbad2987f1a076.rlib" "--extern" "futures=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libfutures-56bbad2987f1a076.rmeta" "--extern" "itertools=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libitertools-49215d6c78da5485.rlib" "--extern" "itertools=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libitertools-49215d6c78da5485.rmeta" "--extern" "libc=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/liblibc-9fd0632ab0adaa16.rlib" "--extern" "libc=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/liblibc-9fd0632ab0adaa16.rmeta" "--extern" "parking_lot=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libparking_lot-cc8a335fe9146be3.rlib" "--extern" "parking_lot=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libparking_lot-cc8a335fe9146be3.rmeta" "--extern" "quote=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libquote-aa12901b2adae6fc.rlib" "--extern" "quote=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libquote-aa12901b2adae6fc.rmeta" "--extern" "regex=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libregex-6ecd666cd642c024.rlib" "--extern" "regex=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libregex-6ecd666cd642c024.rmeta" "--extern" "serde=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libserde-6629799eafe4f0d4.rlib" "--extern" "serde=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libserde-6629799eafe4f0d4.rmeta" "--extern" "syn=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libsyn-b0c36228d90d575f.rlib" "--extern" "syn=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libsyn-b0c36228d90d575f.rmeta" "--extern" "tokio=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libtokio-6ddc89563990f085.rlib" "--extern" "tokio=/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps/libtokio-6ddc89563990f085.rmeta" "-L" "/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/debug/deps" "-L" "/checkout/obj/build/x86_64-unknown-linux-gnu/stage2-tools/x86_64-unknown-linux-gnu/debug/deps" "--edition" "2024"

error: actual output differed from expected
Execute `./x test src/tools/clippy --bless` to update `tests/ui/unit_arg.stderr` to the actual output
--- tests/ui/unit_arg.stderr
+++ <stderr output>
 error: passing a unit value to a function
   --> tests/ui/unit_arg.rs:63:5
... 202 lines skipped ...
 LL |     fn_take_unit(mac!(def));
    |     ^^^^^^^^^^^^^^^^^^^^^^^
-   |
 
 error: passing a unit value to a function
... 2 lines skipped ...
 LL |     fn_take_unit(mac!(func Default::default));
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
-   |
 
 error: passing a unit value to a function
... 2 lines skipped ...
 LL |     fn_take_unit(mac!(nonempty_block Default::default()));
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
-   |
 
 error: aborting due to 13 previous errors
 
---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         1;
LL +     };
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:67:5
   |
LL |     foo(foo(1));
   |     ^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     foo(1);
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:69:5
   |
---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         foo(1);
LL +         foo(2);
LL +     };
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:75:5
   |
---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         1;
LL +     };
LL ~     b.bar(());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:79:5
   |
LL |     taking_multiple_units(foo(0), foo(1));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
help: move the expressions in front of the call and replace them with the unit literal `()`
   |
LL ~     foo(0);
LL +     foo(1);
LL ~     taking_multiple_units((), ());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:81:5
   |
---
   |
LL ~     foo(0);
LL +     {
LL +
LL +         foo(1);
LL +         foo(2);
LL +     };
LL ~     taking_multiple_units((), ());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:86:5
   |
---

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:98:13
   |
LL |     None.or(Some(foo(2)));
   |             ^^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     None.or({
LL +         foo(2);
LL +         Some(())
LL ~     });
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:102:5
   |
LL |     foo(foo(()));
   |     ^^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     foo(());
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:140:5
   |
---

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:171:5
   |
LL |     fn_take_unit(mac!(def));
   |     ^^^^^^^^^^^^^^^^^^^^^^^

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:173:5
   |
LL |     fn_take_unit(mac!(func Default::default));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:175:5
   |
LL |     fn_take_unit(mac!(nonempty_block Default::default()));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: aborting due to 13 previous errors


---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         1;
LL +     };
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:67:5
   |
LL |     foo(foo(1));
   |     ^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     foo(1);
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:69:5
   |
---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         foo(1);
LL +         foo(2);
LL +     };
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:75:5
   |
---
help: or move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     {
LL +
LL +         1;
LL +     };
LL ~     b.bar(());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:79:5
   |
LL |     taking_multiple_units(foo(0), foo(1));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
help: move the expressions in front of the call and replace them with the unit literal `()`
   |
LL ~     foo(0);
LL +     foo(1);
LL ~     taking_multiple_units((), ());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:81:5
   |
---
   |
LL ~     foo(0);
LL +     {
LL +
LL +         foo(1);
LL +         foo(2);
LL +     };
LL ~     taking_multiple_units((), ());
   |

error: passing unit values to a function
##[error]  --> tests/ui/unit_arg.rs:86:5
   |
---

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:98:13
   |
LL |     None.or(Some(foo(2)));
   |             ^^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     None.or({
LL +         foo(2);
LL +         Some(())
LL ~     });
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:102:5
   |
LL |     foo(foo(()));
   |     ^^^^^^^^^^^^
   |
help: move the expression in front of the call and replace it with the unit literal `()`
   |
LL ~     foo(());
LL ~     foo(());
   |

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:140:5
   |
---

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:171:5
   |
LL |     fn_take_unit(mac!(def));
   |     ^^^^^^^^^^^^^^^^^^^^^^^

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:173:5
   |
LL |     fn_take_unit(mac!(func Default::default));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: passing a unit value to a function
##[error]  --> tests/ui/unit_arg.rs:175:5
   |
LL |     fn_take_unit(mac!(nonempty_block Default::default()));
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error: aborting due to 13 previous errors


```

</details>


---

rust-log-analyzer 2025-12-16T11:16:31Z


The job **`pr-check-2`** failed! Check out the build log: [(web)](https://github.com/rust-lang/rust/actions/runs/20265788050/job/58188775647) [(plain enhanced)](https://triage.rust-lang.org/gha-logs/rust-lang/rust/58188775647) [(plain)](https://github.com/rust-lang/rust/commit/2e4968b8c4ca75a6c6173164c4cac8d03a82a441/checks/58188775647/logs)

<details><summary><i>Click to see the possible cause of the failure (guessed by this bot)</i></summary>

```plain
[RUSTC-TIMING] askama test:false 0.165
error: unused import: `HumanEmitter`
  --> src/librustdoc/core.rs:11:17
   |
11 |     DynEmitter, HumanEmitter, HumanReadableErrorType, OutputTheme, stderr_destination,
   |                 ^^^^^^^^^^^^
   |
   = note: `-D unused-imports` implied by `-D warnings`
   = help: to override `-D warnings` add `#[allow(unused_imports)]`

error[E0223]: ambiguous associated type
   --> src/librustdoc/doctest.rs:628:31
    |
628 |         let unicode = kind == HumanReadableErrorType::AnnotateSnippet { unicode: true, short };
    |                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
help: if there were a trait named `Example` with associated type `AnnotateSnippet` implemented for `HumanReadableErrorType`, you could use the fully-qualified path
    |
628 -         let unicode = kind == HumanReadableErrorType::AnnotateSnippet { unicode: true, short };
628 +         let unicode = kind == <HumanReadableErrorType as Example>::AnnotateSnippet { unicode: true, short };
    |

For more information about this error, try `rustc --explain E0223`.
[RUSTC-TIMING] rustdoc test:false 3.331
error: could not compile `rustdoc` (lib) due to 2 previous errors
```

</details>


---

rustbot 2025-12-16T13:26:53Z

`rustc_errors::annotate_snippet_emitter_writer` was changed

cc @Muscraft

`rustc_errors::emitter` was changed

cc @Muscraft

Some changes occurred in src/tools/clippy

cc @rust-lang/clippy

---

rustbot 2025-12-16T13:26:56Z

r? @JonathanBrouwer

rustbot has assigned @JonathanBrouwer.
They will have a look at your PR within the next two weeks and either review your PR or reassign to another reviewer.

Use `r?` to explicitly pick a reviewer

---

Kivooeo 2025-12-16T13:37:36Z

@Muscraft, feel free to assign yourself on this

I also took a look if documentation should be adjusted for this change and has found nothing related, so it seems like this is the only needed change to close tracking issue

---

JonathanBrouwer 2025-12-16T15:18:09Z

I'm happy to review this, but Muscraft seems like the better option here.
r? @Muscraft 
If you don't have capacity to review this or any other reason feel free to assign this back to me :)

---

Muscraft 2025-12-16T17:21:57Z

@bors r+ rollup

---

bors 2025-12-16T17:22:01Z

:pushpin: Commit 84f2854bc3bfea59aeb6fd23245a788196c9d4ff has been approved by `Muscraft`

It is now in the [queue](https://bors.rust-lang.org/homu/queue/rust) for this repository.

<!-- @bors r=Muscraft 84f2854bc3bfea59aeb6fd23245a788196c9d4ff -->
<!-- homu: {"type":"Approved","sha":"84f2854bc3bfea59aeb6fd23245a788196c9d4ff","approver":"Muscraft","queue":"https://bors.rust-lang.org/homu/queue/rust"} -->

