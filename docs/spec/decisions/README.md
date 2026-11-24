# Decisions

One file per decision, `<number>.md`: its title, its facts (`Status`, `Where` the spec files that state it, `Amended by`, `Superseded by`, `Note`) and its text. Numbers are never reused, and code cites them as `decision <number>`. The questions the first decisions settled are in [questions](questions.md). This index is written by `cargo xtask decisions`.

Numbers 184 to 189, 217 to 220, 369 to 379, 452 to 469, 521 to 539, 544 to 559, 562 to 579, 581 to 599, 603 to 619, 621 to 639, 643 to 644 were never assigned.

## Proposed

| # | Decision | Where |
|---|----------|-------|
| [658](658.md) | Commands show progress | 12 |
| [684](684.md) | The command line has a closed vocabulary of verbs | 12 |
| [685](685.md) | `vun check` reports wide intermediates and generic code size on request | 03, 04, 09, 13 |
| [686](686.md) | A link symbol with a pack is imported as a variadic C function | 07, 13 |

## All

| # | Decision | Status | Where |
|---|----------|--------|-------|
| [1](001.md) | Integers are ranges | accepted | 04 |
| [2](002.md) | Scoped borrows, no lifetime annotations | accepted | 05 |
| [3](003.md) | `.` operates on places | accepted | 01 |
| [4](004.md) | Async is first-class through one suspension primitive shared with iterators | accepted | 06 |
| [5](005.md) | No builtins | accepted | 00, 01 |
| [6](006.md) | Pluggable backends and linkers | accepted | 13 |
| [7](007.md) | `fixed` kind instead of `Pin` | accepted | 05 |
| [8](008.md) | Safe interior mutability is always synchronized, so no `Send`/`Sync` | accepted | 07 |
| [9](009.md) | `defer` plus place states replaces `errdefer` | accepted | 05 |
| [10](010.md) | One loop keyword, `for` | accepted | 01 |
| [11](011.md) | 15 keywords | accepted | 01 |
| [12](012.md) | Interfaces are functions from a type to a requirement struct | accepted | 02 |
| [13](013.md) | Programs are their exports | accepted | 08 |
| [14](014.md) | Coroutines take no resume input | accepted | 06 |
| [15](015.md) | Metaprogramming in four layers | accepted | 03 |
| [16](016.md) | Initialization in place | accepted | 05 |
| [17](017.md) | Aliasing facts are part of the language | accepted | 05, 09 |
| [18](018.md) | Every entity in every IR carries an origin chain | accepted | 10, 13 |
| [19](019.md) | Diagnostic ids are binding paths | accepted | 10 |
| [20](020.md) | Manifest is `build.vun` with a literal `package` | accepted | 12 |
| [21](021.md) | Tests are `#[test]` bindings | accepted | 11 |
| [22](022.md) | Package namespace is `<name>.vun` plus `<name>/`, merged | accepted | 01, 12 |
| [23](023.md) | CST is a postorder array anchored one token per node | accepted | 01, 13 |
| [24](024.md) | A backslash takes what follows literally | accepted | 01 |
| [25](025.md) | `static` is contextual after `&`, like `out` and `take` | accepted | 01, 05 |
| [26](026.md) | Command lines are declared as data | accepted | 13 |
| [27](027.md) | Literal data reads tagged values `.name(a, b)` | accepted | 13 |
| [28](028.md) | Struct and enum literals are nominal | accepted | 02, 03 |
| [29](029.md) | `match` is exhaustive | accepted | 01 |
| [30](030.md) | Pattern forms reuse operators | accepted | 01 |
| [31](031.md) | Matching through a borrow binds with the scrutinee's mode | accepted | 01, 05 |
| [32](032.md) | No shadowing within a file | accepted | 01 |
| [33](033.md) | The manifest names the prelude | accepted | 01, 12 |
| [34](034.md) | Lowercase primitive and prelude type names are an exemption list of | accepted | 10 |
| [35](035.md) | Integer comparisons narrow the range types of immutable bindings in the region | accepted | 04 |
| [36](036.md) | Typed literals stay `T{ }` with their position rule | accepted | 01 |
| [37](037.md) | `&temp` borrows never flow into the result, so the result is not tied to them | accepted | 01, 05 |
| [38](038.md) | Integer intermediates are capped at 128 bits on every target | accepted | 04 |
| [39](039.md) | Safe shared ownership is always synchronized | accepted | 07 |
| [40](040.md) | Trees are built in arenas | accepted | 05 |
| [41](041.md) | Typed handles are a library | accepted | 05 |
| [42](042.md) | Registrations are collected explicitly | accepted | 03 |
| [43](043.md) | Text is a library | accepted | 12 |
| [44](044.md) | The leans of questions 4 to 9 and 11 to 13 are accepted as stated below | accepted | various |
| [45](045.md) | Integer types by width are `lang.int.unsigned(bits)` and `lang.int.signed(bits)` | accepted | 04 |
| [46](046.md) | Spreads `..path;` and `..path.{ a, b };` re-export the public members of a | accepted | 01 |
| [47](047.md) | A compile-time block binding is visible in its own value | accepted | 01 |
| [48](048.md) | A directory holds a package when its `build.vun` declares `package` | accepted | 12 |
| [49](049.md) | `targets` is a default dependency, like `lang` and `std` | accepted | 01, 12 |
| [50](050.md) | BLAKE3 everywhere | accepted | 12 |
| [51](051.md) | Packages are published as `.ovpkg` | accepted | 12 |
| [52](052.md) | Errors keep their source chains | accepted | 10 |
| [53](053.md) | `vun pkg add` trusts an archive on first use and writes its entry as a literal | accepted | 12 |
| [54](054.md) | A snapshot entry records whether its file is executable | accepted | 12 |
| [55](055.md) | Minimal version selection runs over the packages present | accepted | 12 |
| [56](056.md) | One store per user in the XDG cache directory | accepted | 12 |
| [57](057.md) | The query engine memoizes with verifying traces of interface hashes | accepted | 13 |
| [58](058.md) | Symbols come from one process-wide, sharded, append-only pool | accepted | 13 |
| [59](059.md) | Stable identities | accepted | 10, 13 |
| [60](060.md) | `vun pkg update` reads versions from an index speaking a Go-proxy-style protocol | accepted | 12 |
| [61](061.md) | The query database persists under `.vun/queries/`, keyed by the toolchain's | accepted | 13 |
| [62](062.md) | Explanation pages stay explicit data | superseded | 10 |
| [63](063.md) | Binary caches are declared in the root manifest | accepted | 12 |
| [64](064.md) | `.replace` in the root manifest substitutes a path or a hash for an identity | accepted | 12 |
| [65](065.md) | A partial graph | accepted | 10, 12 |
| [66](066.md) | The compiler's primitives are the top-level `= _` declarations of `lang` | accepted | 00, 13 |
| [67](067.md) | Dependencies are pure Rust wherever a production-grade crate exists or the | accepted | 12, 13 |
| [68](068.md) | `lang.Fn(params, result)` is the primitive function type, `(A, B) -> R` being | accepted | 02, 04, appendix A |
| [69](069.md) | Appendix B is data | accepted | 02, appendix B |
| [70](070.md) | Visibility levels | accepted | 01 |
| [71](071.md) | Conditions are data | accepted | 01, appendix B |
| [72](072.md) | Compound assignments are data | accepted | 03, 04, appendix B |
| [73](073.md) | Compile-time traps are diagnostics | accepted | 03 |
| [74](074.md) | Typing uses the expected type where the context gives one | accepted | 01, 04 |
| [75](075.md) | Type syntax is complete in evaluation | accepted | 02, appendix B |
| [76](076.md) | An impl's `T` may be a type constructor | accepted | 01, 02 |
| [77](077.md) | A function value is its literal and the compile-time call it was evaluated in | accepted | 03, 04 |
| [78](078.md) | `[N]T` and `[T]` are `lang.mem.Array(N, T)` and `lang.mem.Slice(T)` | accepted | 02, 05, appendix B |
| [79](079.md) | `a..b`, `a..=b` and `a..` have type `lang.Bounds(T)`, `T` the integers every | accepted | 04, appendix B |
| [80](080.md) | Loops are typed | accepted | 01, appendix B |
| [81](081.md) | `&e` is typed through the table `borrows` | accepted | 01, 04 |
| [82](082.md) | Compile-time evaluation reads types | accepted | 02, 03, 13 |
| [83](083.md) | `$for` and `$if` in type bodies are expanded when the members are read | accepted | 03 |
| [84](084.md) | Reflection descriptors are ordinary `lang.meta` structs named by each | accepted | 03 |
| [85](085.md) | `$for` and `$if` among statements generate statements | accepted | 03 |
| [86](086.md) | Code values are `lang.meta.Code(T)` values holding the expression and what it | accepted | 03 |
| [87](087.md) | Quote levels | accepted | 03 |
| [88](088.md) | MIR places carry projections | accepted | 13 |
| [89](089.md) | Patterns lower to tests in MIR | accepted | 01, 13 |
| [90](090.md) | A top-level declaration | accepted | 08, 13 |
| [91](091.md) | `e?` goes through the table `early_return` | accepted | 01, 02, appendix B |
| [92](092.md) | Coroutines | accepted | 02, 06, appendix B |
| [93](093.md) | Layout as spec 05 section 6 states it, computed by the query `Layouts` | accepted | 05 |
| [94](094.md) | Place states are checked on MIR | accepted | 05, 13 |
| [95](095.md) | Kinds | accepted | 05 |
| [96](096.md) | `defer s;` is lowered again at every exit of its block | accepted | 05, 13 |
| [97](097.md) | Borrow rules on MIR | accepted | 05 |
| [98](098.md) | `#[kind.free]` makes a type free | accepted | 05, appendix A |
| [99](099.md) | A `$` parameter named in a later parameter's type may be omitted from a call | accepted | 02, 07 |
| [100](100.md) | `.` looks through owners as through borrows | accepted | 01, 05 |
| [101](101.md) | Two-phase receivers | accepted | 01, 05 |
| [102](102.md) | `#[impl(T)]` without an interface | accepted | 02 |
| [103](103.md) | `true` and `false` are declarations of `lang` | accepted | 02 |
| [104](104.md) | A block's tail may carry attributes and docs | accepted | 01, 05 |
| [105](105.md) | An `enum` literal holds declarations after its variants and a `;`, which are | accepted | 01, 02 |
| [106](106.md) | Function types stay plain function values | accepted | 02, appendix B |
| [107](107.md) | Raw pointer primitives in `lang.mem` | accepted | 03, 05, 07, appendix A |
| [108](108.md) | A value of an empty type such as `never` is accepted wherever any type is | accepted | 02 |
| [109](109.md) | `unsafe` is a single marker | accepted | 07 |
| [110](110.md) | `lang.hint.unreachable` says control never gets here | accepted | 09 |
| [111](111.md) | Every spec 07 section 4 property is a marker of the function type | accepted | 07, appendix B |
| [112](112.md) | `lang.int.magnitude($r) -> Range` | accepted | 01, 04 |
| [113](113.md) | Rule 8 as typing | accepted | 04 |
| [114](114.md) | `coroutine.recursive_frame` is reported at a coroutine function whose body | accepted | 06 |
| [115](115.md) | The freeze rule is checked on declarations | accepted | 03 |
| [116](116.md) | Structural records | accepted | 02 |
| [117](117.md) | `std.build` | accepted | 02, 12 |
| [118](118.md) | No bespoke formats in the ecosystem | accepted | 12 |
| [119](119.md) | `&temp T` and `&temp mut T` are rows of the table `borrows` | accepted | 05, appendix B |
| [120](120.md) | The language is named Vun | accepted | all |
| [121](121.md) | `std.sync` in Vun | accepted | 05, 07, 08 |
| [122](122.md) | A top-level binding reached as a place is its program-long storage | accepted | 01, 02, 13 |
| [123](123.md) | `&static T` is the row `static` of `borrows` | accepted | 01, 02, 05 |
| [124](124.md) | `std.fmt` | accepted | 03, 08 |
| [125](125.md) | Library diagnostics | accepted | 03, 10 |
| [126](126.md) | Executors in `std.task` | accepted | 06 |
| [127](127.md) | `std.iter` adapters | accepted | 06 |
| [128](128.md) | A compile-time call's argument that is known not to be a value of its | accepted | 03, 05 |
| [129](129.md) | Runtimes export `std.sync.enter`/`leave` and `std.task.sleep` | accepted | 07, 08 |
| [130](130.md) | `#[layout.c]` struct | accepted | 05 |
| [131](131.md) | `vun` is one executable | accepted | 13 |
| [132](132.md) | The generator is its own binary, `vun-generate` | accepted | 13 |
| [133](133.md) | Rust names declarations of `lang` and `std` through generated handles | accepted | 13 |
| [134](134.md) | `vun tool bind` reads a header through libclang with the build's flags | accepted | 07, 12 |
| [135](135.md) | Compile-time `&mut` | accepted | 03 |
| [136](136.md) | A package's plan | accepted | 03, 12 |
| [137](137.md) | A shared borrow of what lives as long as the program is `&static` | accepted | 03, 05 |
| [138](138.md) | Targets and lint groups are data | accepted | 10, 13 |
| [139](139.md) | `cargo xtask size` builds `vun-cli` in the `dist` profile and fails past | accepted | 13 |
| [140](140.md) | `std` additions | accepted | 05, 06, 08 |
| [141](141.md) | Values and members | accepted | 01, 02, 03, 07 |
| [142](142.md) | `lang.lint` | accepted | 10 |
| [143](143.md) | Compile-time calls | accepted | 03, 13 |
| [144](144.md) | Every operator is its interface's function for the left operand's type | accepted | 02, 04, 13 |
| [145](145.md) | Layout placement | accepted | 05, 13 |
| [146](146.md) | A `yield` is an exit | accepted | 06, 13 |
| [147](147.md) | A member a namespace does not declare is `names.unresolved` | accepted | 10 |
| [148](148.md) | Programs and concrete bodies | accepted | 08, 13 |
| [149](149.md) | Typed, never spelled | accepted | 13, appendix B |
| [150](150.md) | LIR | accepted | 13 |
| [151](151.md) | Lowering MIR to LIR | accepted | 04, 13 |
| [152](152.md) | Backends after TPDE | accepted | 13 |
| [153](153.md) | The C backend writes strict C99 | accepted | 13 |
| [154](154.md) | Targets are named `targets.<system>.<cpu>` | accepted | 13 |
| [155](155.md) | Coroutine frames are specified | accepted | 06, 13 |
| [156](156.md) | Comparisons narrow | accepted | 04, 06, 13, appendix B |
| [157](157.md) | The baseline backend | accepted | 13 |
| [158](158.md) | A coroutine function's frame type is `lang.meta.Frame(f)` | accepted | 03, 06 |
| [159](159.md) | A trap site reached at compile time traps, as at run time | accepted | 03, 07 |
| [160](160.md) | Calls read their arguments one way | accepted | 03, 07, 13 |
| [161](161.md) | The baseline backend fuses a compare into the branch that is its one use, keeps | accepted | 13 |
| [162](162.md) | `stack.max` is checked when the program is built | accepted | 07, 13 |
| [163](163.md) | An attribute's type says where it applies | accepted | 02 |
| [164](164.md) | The declarations of one link symbol that a program sees | accepted | 07 |
| [165](165.md) | A namespace's link symbol names are the members of its `symbols` struct, beside | accepted | 08 |
| [166](166.md) | The baseline backend holds up to five values live across the blocks of a loop | accepted | 13 |
| [167](167.md) | Floats | accepted | 04, 13 |
| [168](168.md) | Commands are organized as jj's | accepted | 12, 13 |
| [169](169.md) | Integers wider than a backend computes with are legalized in LIR | accepted | 13 |
| [170](170.md) | Lowering covers every run-time form | accepted | 05, 13 |
| [171](171.md) | A path naming a member its namespace lacks is reported wherever it is written | accepted | 01, 13 |
| [172](172.md) | Reflection gives every nominal type one shape | accepted | 03 |
| [173](173.md) | NaN is defined | accepted | 04, 13 |
| [174](174.md) | Operations no instruction computes are Vun | accepted | 04, 13 |
| [175](175.md) | `f16` is a LIR float like the others | accepted | 13 |
| [176](176.md) | The query engine | accepted | 13 |
| [177](177.md) | Query persistence | accepted | 13 |
| [178](178.md) | Origins are equal and hashed by their anchored identity | accepted | 10, 13 |
| [179](179.md) | `.vun/` is the toolchain's directory beside sources | accepted | 12 |
| [180](180.md) | Timings | accepted | 13 |
| [181](181.md) | Output and command line redesigned after cargo and jj | accepted | 12, 13 |
| [182](182.md) | Backends are chosen by profile | accepted | 13 |
| [183](183.md) | The native backend is layered as TPDE is, one crate per layer | accepted | 13 |
| [190](190.md) | Machine-readable output | accepted | 10, 12 |
| [191](191.md) | Saved queries stay bounded and whole | accepted | 12, 13 |
| [192](192.md) | The native backend generates arm64 | accepted | 12, 13 |
| [193](193.md) | The size budget of `cargo xtask size` is 12 MiB | accepted | 13 |
| [194](194.md) | Or-patterns that bind | accepted | 01, 13 |
| [195](195.md) | A HIR node in a query key | accepted | 13 |
| [196](196.md) | Exhaustive `match` | accepted | 01 |
| [197](197.md) | Saved queries are loaded as they are used | accepted | 13 |
| [198](198.md) | Cycles among queries | accepted | 13 |
| [199](199.md) | `#[impl(I(T))]` checks each provided requirement's type | accepted | 02 |
| [200](200.md) | Lowering promotes slots to values | accepted | 13 |
| [201](201.md) | Native code quality | accepted | 13 |
| [202](202.md) | The LIR builder folds integer operations, comparisons and conversions of | accepted | 13 |
| [203](203.md) | At a call, the native backend stores home only what is used after it | accepted | 13 |
| [204](204.md) | The native backend writes DWARF 4 line tables | accepted | 10, 13 |
| [205](205.md) | The native backend names an internal function's symbol by the declaration it is | accepted | 13 |
| [206](206.md) | `vun-object` writes Mach-O 64-bit objects | accepted | 13 |
| [207](207.md) | The native backend's instruction compilers take the target's variant of its C | accepted | 13 |
| [208](208.md) | The mold and wild adapters link ELF through the target's C compiler, sharing | accepted | 12, 13 |
| [209](209.md) | `lang.target.CCompiler` gains | accepted | 13 |
| [210](210.md) | A run-time binding holding a slice `[T]` by value is reported by the typer as | accepted | 02 |
| [211](211.md) | After promotion, lowering threads jumps | accepted | 13 |
| [212](212.md) | Lowering simplifies each function once built | accepted | 13 |
| [213](213.md) | A constant passed in a general register is written there already extended as | accepted | 13 |
| [214](214.md) | Freeing a fixed register an instruction needs | accepted | 13 |
| [215](215.md) | The native backend writes `.eh_frame` for ELF objects | accepted | 13 |
| [216](216.md) | `vun build` on a package's `build.vun` carries out its plan | accepted | 12 |
| [221](221.md) | Compile-time calls run on stack the evaluator adds in 16 MiB segments whenever | accepted | 03, 13 |
| [222](222.md) | Builds keep what they make by content | accepted | 13 |
| [223](223.md) | A compile with no saved queries parses every file of the published packages in | accepted | 13 |
| [224](224.md) | What every compile-time call reads of its function is memoized | accepted | 13 |
| [225](225.md) | A query table stores each key with its hash, computed once | accepted | 13 |
| [226](226.md) | Lowering merges a block into its only predecessor ending in a jump to it | accepted | 13 |
| [227](227.md) | Simplification runs callees first, so a function inlined has its own small | accepted | 13 |
| [228](228.md) | Simplification scalarizes aggregate slots reached only by constant-offset | accepted | 13 |
| [229](229.md) | Jump threading also goes through a block switching or branching on a parameter | accepted | 13 |
| [230](230.md) | A block jumping to itself heads a loop of its own | accepted | 13 |
| [231](231.md) | Dense sets of indices live in `vun-index` | accepted | 13 |
| [232](232.md) | Comparison operators are `lang.ops.PartialOrd` | accepted | 02, appendix B |
| [233](233.md) | The order of an atomic access is `lang.mem.Order` | accepted | 05, 07 |
| [234](234.md) | `std.fmt.Writer` requires | accepted | 02 |
| [235](235.md) | `std.sync.Shared(T)` is reached only by value under a `&Critical` | accepted | 07 |
| [236](236.md) | `targets.gnu` takes the flags selecting a target's machine | accepted | 13 |
| [237](237.md) | Compile-time values are bounded | accepted | 03 |
| [238](238.md) | Executors only look at an `Event`'s mark | accepted | 06 |
| [239](239.md) | `lang.target.current` is the chosen target as a compile-time value | accepted | 13 |
| [240](240.md) | `lang.meta.same($T, a, b)` says whether two compile-time values are one value | accepted | 03 |
| [241](241.md) | Saved query memos carry the identity of the compiler build that wrote them | accepted | 13 |
| [242](242.md) | A package's `include` names `build.vun`, so its hash covers its manifest | accepted | 12 |
| [243](243.md) | The place, borrow and panic checks run on every function of a file and on every | accepted | 07, 13 |
| [244](244.md) | An impl's member must carry every marker its requirement declares | accepted | 02, 07 |
| [245](245.md) | LIR `Offset` stays inside the object its base points into, or one past its end | accepted | 13 |
| [246](246.md) | An integer wider than a register is a typed wide `AbiType` in LIR, not split by | accepted | 13 |
| [247](247.md) | A file's persisted key is typed | accepted | 10, 13 |
| [248](248.md) | Network sources are `https://` only | accepted | 12 |
| [249](249.md) | `lang.link.keep` is removed | accepted | 08, appendices |
| [250](250.md) | `std.mem.alloc.Fixed` borrows its buffer | accepted | 07 |
| [251](251.md) | What a C library lays out its own way is a row per system | accepted | 08 |
| [252](252.md) | The size budget of the distributed `vun` is 20 MiB | accepted | 13 |
| [253](253.md) | A wasm backend of the toolchain's own | accepted | 13 |
| [254](254.md) | `lang.link.library(name)` names the library a linked symbol | accepted | 08, 13 |
| [255](255.md) | Positions are shown through a compilation's view of the files it reads | accepted | 10 |
| [256](256.md) | Calling conventions are data in one crate, `vun-abi` | accepted | 13 |
| [257](257.md) | The native backends share a register allocator, `vun-regalloc` | accepted | 13 |
| [258](258.md) | Inline assembly is a block of an architecture's instructions | accepted | 07, 13 |
| [259](259.md) | A field is not a method | accepted | 01 |
| [260](260.md) | Compile-time work in a body splits into expansions | accepted | 03, 04 |
| [261](261.md) | A float literal that nothing gives a float type is an `f64` | accepted | 03, 04 |
| [262](262.md) | The formatter changes whitespace only and breaks lines past a width | accepted | 12, 13 |
| [263](263.md) | `std.os.start` is the `start` of the runtime of the target being built for | accepted | 08 |
| [264](264.md) | What a program is built as is `std.build.Output` | accepted | 08, 12, 13 |
| [265](265.md) | Floats are one constructor, `lang.Float($bits)` | accepted | 02, 04 |
| [266](266.md) | Exit statuses are `std.os.Status := Int(1..=125)`, the range every supported | accepted | 08 |
| [267](267.md) | A failing file is reduced by `vun-reduce` | accepted | 13 |
| [268](268.md) | Unsafe code can be checked at run time, by kind | accepted | 07, 13 |
| [269](269.md) | The language server, `vun util lsp` | accepted | 12, 13 |
| [270](270.md) | HTML is typed | accepted | 12 |
| [271](271.md) | A target states the instruction set its code is written in | accepted | 07, 13 |
| [272](272.md) | A diagnostic's explanation page is found by its binding path, as a namespace's | accepted | 10 |
| [273](273.md) | An integer in a literal or range pattern is an operand of the comparison the | accepted | 01, 02 |
| [274](274.md) | A `$` form that surely runs is an error when it cannot be computed where it is | accepted | 03, 10 |
| [275](275.md) | Each branch of a staged `$if` among statements is a scope of its own, as a | accepted | 03 |
| [276](276.md) | The C backend writes a naked function under GNU attributes | accepted | 07, 13 |
| [277](277.md) | `vun-object` writes ELF of the class the machine's address width fixes | accepted | 13 |
| [278](278.md) | A field's `#[layout.offset(N)]` lies at or past the end of the field before it | accepted | 05 |
| [279](279.md) | Linux targets are static by default | accepted | 08 |
| [280](280.md) | `vun-abi` places 32-bit Arm's conventions through the same `Rules` as the others | accepted | 13 |
| [281](281.md) | Operations of the system beyond writing reach it through link symbols each | accepted | 08 |
| [282](282.md) | A borrow serves where another is expected when it performs the same transition | accepted | 05 |
| [283](283.md) | Every value hashes by what it compares by | accepted | 13 |
| [284](284.md) | Files through the runtimes' link symbols | accepted | 08, 13 |
| [285](285.md) | Packs | accepted | 01, 02, appendix B |
| [286](286.md) | Variadic C functions are C imports with a pack, declared with the pack of the | accepted | 07, 13 |
| [287](287.md) | The libraries' memos are shared between projects | accepted | 13 |
| [288](288.md) | `vun-par` runs on one pool of threads for the process, lending a call's work | accepted | 13 |
| [289](289.md) | A repeated item | accepted | 13 |
| [290](290.md) | LIR is lowered per function | accepted | 13 |
| [291](291.md) | Reusing memos, faster | accepted | 13 |
| [292](292.md) | Legalization splits every integer wider than the backend's widest into limbs of | accepted | 13 |
| [293](293.md) | Native Thumb-2 | accepted | 07, 13 |
| [294](294.md) | Floats a processor does not compute are Vun | accepted | 01, 04, 13 |
| [295](295.md) | A local declared with a value existing at compile time only | accepted | 03 |
| [296](296.md) | Conversions between integers and floats and between float widths are explicit | accepted | 04, 13 |
| [297](297.md) | A float the backend does not hold | accepted | 04, 13 |
| [298](298.md) | Atomic accesses of integers wider than an address are `#[lower.soft]` to | accepted | 05, 13 |
| [299](299.md) | `lang.target.Spec.register_bits`, a general register's width, is the widest | accepted | 13 |
| [300](300.md) | The float bank of the register allocator is the home of values no general | accepted | 13 |
| [301](301.md) | Introspection of reuse and threads | accepted | 13 |
| [302](302.md) | Durability | accepted | 13 |
| [303](303.md) | Inlining per function | accepted | 13 |
| [304](304.md) | Layered memo files | accepted | 13 |
| [305](305.md) | Workers join calls in flight | accepted | 13 |
| [306](306.md) | Threads | accepted | 08, 13 |
| [307](307.md) | Kernel targets link `-static -nostdlib` and name the C compiler's helper | accepted | 08, 13 |
| [308](308.md) | A target lists what its processor has of its instruction set beyond what every | accepted | 07, 13 |
| [309](309.md) | Backtraces | accepted | 07, 08, 13 |
| [310](310.md) | Native code is compiled for a processor | accepted | 07, 13 |
| [311](311.md) | Division without a division instruction | accepted | 13 |
| [312](312.md) | Storage per thread is a property of a type | accepted | 05, 07, 08, 13 |
| [313](313.md) | A thread's backtrace names the calls that spawned it | accepted | 07 |
| [314](314.md) | How a platform reaches storage per thread is part of its calling convention | accepted | 07 |
| [315](315.md) | `std.fmt.print($W, w, $format, ..args)` takes its arguments as a pack | accepted | 03, 08 |
| [316](316.md) | Structured data is read and written through typed values | accepted | 13 |
| [317](317.md) | URLs are typed values | accepted | 12 |
| [318](318.md) | The language server's messages are typed | accepted | 13 |
| [319](319.md) | Each node kind is declared by its production in `grammar.vun` | accepted | 13 |
| [320](320.md) | Structured data is one model apart from any format | accepted | 13 |
| [321](321.md) | Editors read Vun through a tree-sitter grammar generated from `nodes.vun` and | accepted | 13 |
| [322](322.md) | A declaration's origin is identified by its name inside the declaration around | accepted | 10 |
| [323](323.md) | What the generator interprets of the grammar, the productions and the tables | accepted | 13 |
| [324](324.md) | The language server answers references, rename, code actions, signature help | accepted | 13 |
| [325](325.md) | The native backend's DWARF unit holds an entry per function | accepted | 10, 13 |
| [326](326.md) | A program's build places its sections | accepted | 08, 12, 13 |
| [327](327.md) | An exported value that is not a function is data of the program | accepted | 08, 13 |
| [328](328.md) | A constant shifted by an amount known only at run time has no width | accepted | 04 |
| [329](329.md) | Board peripherals are tokens `Peripherals` holds, made by an `unsafe` claim | accepted | 08 |
| [330](330.md) | macOS links through lld's Mach-O flavor | accepted | 12, 13 |
| [331](331.md) | `x.wrap(T)` reduces modulo the size of `T` for every finite `T`, as | accepted | 04 |
| [332](332.md) | `$for` and `$if` in an array or tuple literal generate its items | accepted | 03 |
| [333](333.md) | `lang.simd.Vector(n, T)` is laid out as `[n]T`, aligned to its size when a | accepted | 03, 04 |
| [334](334.md) | Compile-time evaluation while a body is typed reads everything typing found so | accepted | 03 |
| [335](335.md) | `lang.meta.embed(path)` reads a file the package includes, from the directory | accepted | 03 |
| [336](336.md) | `$if` tests a pattern as `if` does | accepted | 03 |
| [337](337.md) | Library reports point inside literals through code | accepted | 10 |
| [338](338.md) | Lints run | accepted | 10 |
| [339](339.md) | `std.lints.naming.case` | accepted | 10 |
| [340](340.md) | The parser reads the productions of `nodes.vun` and replaces the hand parser | accepted | 13 |
| [341](341.md) | The shadow of the checks of unsafe code follows a program's memory wherever the | accepted | 13 |
| [342](342.md) | `--format styled` | accepted | 12 |
| [343](343.md) | Inline assembly is ordinary Vun, replacing 258's block syntax | accepted | 01, 02, 07, 13 |
| [344](344.md) | `lang.diag.error` reached at run time refuses the program | accepted | 07, 10 |
| [345](345.md) | An impl whose witness is made for a use, by a type constructor or for the | accepted | 02 |
| [346](346.md) | The members a spread brings into a `struct` or `enum` body are members of its | accepted | 01 |
| [347](347.md) | Every editor's queries are written general patterns first, the last matching | accepted | 13 |
| [348](348.md) | Measuring speed, permanently | accepted | 13 |
| [349](349.md) | The language server shows every representation the compiler builds and uses | accepted | 13 |
| [350](350.md) | Diagnostics are written to one rule | accepted | 10 |
| [351](351.md) | Lints see locals | accepted | 10 |
| [352](352.md) | Attributes are reflected | accepted | 03 |
| [353](353.md) | Namespaces are reflected | accepted | 03 |
| [354](354.md) | A trap ends its thread | accepted | 07 |
| [355](355.md) | Tests run as a program the library makes | accepted | 11, 12, 03, 07 |
| [356](356.md) | Every command taking a root file takes a path instead, optional | accepted | 12 |
| [357](357.md) | `#[test.ignore("why")]` is a call of `std.testing.test.ignore`, giving a | accepted | 11, 13 |
| [358](358.md) | Calls and `$` arguments, three gaps closed without a new rule | accepted | 02, 13 |
| [359](359.md) | Typing gives every form of a body a type or says why | accepted | 02, 13 |
| [360](360.md) | Typing says why in generic bodies too | accepted | 02, 13 |
| [361](361.md) | Property tests run | accepted | 02, 03, 07, 11 |
| [362](362.md) | Code written in a body is identified by the instantiation evaluating it, not | accepted | 02, 13 |
| [363](363.md) | Property tests' gaps | accepted | 03, 11 |
| [364](364.md) | A toolchain is a directory holding `bin/vun` and its library in `lib/` | accepted | 12 |
| [365](365.md) | Every target is at a path of one rule, `targets.<os>.<arch>.<abi>` | accepted | 12, 13 |
| [366](366.md) | One name per concept, and a command showing each | accepted | 12, 13 |
| [367](367.md) | Toolchains are installed, pinned and updated as Go's are, with no launcher apart | accepted | 12 |
| [368](368.md) | A release's manifest is signed and pinned | accepted | 12 |
| [380](380.md) | What a program reaches in other packages is checked as its own code | accepted | 04, 11 |
| [381](381.md) | A plain name in a pattern always binds | accepted | 01, 10 |
| [382](382.md) | `lang.int.least(r)` and `lang.int.greatest(r)` give a bounded range's ends | accepted | 04, 11 |
| [383](383.md) | Generic bodies are not typed abstractly | accepted | 02 |
| [384](384.md) | Benchmarks run | accepted | 11 |
| [385](385.md) | Workspaces | accepted | 12 |
| [386](386.md) | Simulations run | accepted | 11 |
| [387](387.md) | Coverage-guided fuzzing | accepted | 11 |
| [388](388.md) | The wasm target links in process | accepted | 13 |
| [389](389.md) | The compiler runs under WASI | accepted | 12, 13 |
| [390](390.md) | Indexing takes an integer or bounds | accepted | 02 |
| [391](391.md) | `naming.case` decides what a declaration binds by how it is written, never by | accepted | 10 |
| [392](392.md) | A compile-time report is a `lang.diag.Report` | accepted | 10 |
| [393](393.md) | An interface marked `#[derive(f)]` | accepted | 02 |
| [394](394.md) | Derived impls reach tuples | accepted | 12 |
| [395](395.md) | `vun fmt` breaks what runs past a width, 110 columns unless asked another | accepted | 13 |
| [396](396.md) | The options of commands take their values from, in order, the command line, the | accepted | 12 |
| [397](397.md) | A place has the type typing found for its expression | accepted | 1 |
| [398](398.md) | A place of an argument in a format string is `{}`, written by the argument's | accepted | 3, 10 |
| [399](399.md) | A local is a binder a body binds | accepted | 1 |
| [400](400.md) | An array has the impls of the slice of its items | accepted | 2 |
| [401](401.md) | `impl I for T { members }`, `impl T { members }` and `impl I for T = value;` | accepted | 2 |
| [402](402.md) | A method that is another witness's, as `Debug` of an integer is `Format`'s, is | accepted | 2 |
| [403](403.md) | A function literal that is the value of a declaration annotated with a function | accepted | 2 |
| [404](404.md) | A function literal that is a member of the witness an impl declares and leaves | accepted | 2 |
| [405](405.md) | A function written as an argument, a closure or a literal without a capture | accepted | 1 |
| [406](406.md) | A call checks the bound of each `$` parameter annotated with an interface | accepted | 2 |
| [407](407.md) | A call of a function every parameter of which is a `$` one, or of a primitive | accepted | 04 |
| [408](408.md) | `vun debug disasm <file> [symbol]` lists the executable sections of an ELF file | accepted | 12, 13 |
| [409](409.md) | A `$c` splicing code that has no value at compile time runs where it is | accepted | 03 |
| [410](410.md) | A function literal that is a member of a witness and writes no result, whose | accepted | 02, 06 |
| [411](411.md) | Microsoft x64 is a convention as data | accepted | 13 |
| [412](412.md) | `windows.x86_64.msvc` is a target | accepted | 08, 12, 13 |
| [413](413.md) | `vun debug disasm` reads PE files beside ELF ones | accepted | 12, 13 |
| [414](414.md) | `std.re` holds what a program reading binaries builds on | accepted | 12 |
| [415](415.md) | The flags a C compiler takes for the code of a shared library are target data | accepted | 08, 13 |
| [416](416.md) | A program loads libraries as it runs | accepted | 05, 08 |
| [417](417.md) | A function or function type names a calling convention of the target's | accepted | 07, 13 |
| [418](418.md) | `windows.x86.msvc` is a target | accepted | 08, 13 |
| [419](419.md) | Lowering stops at nothing the front end could say | accepted | 04, 10, 13 |
| [420](420.md) | What a form needs when compiling reads nothing the body computes when it runs | accepted | 03, 08, 13 |
| [421](421.md) | The command line sets lint levels | accepted | 10, 12 |
| [422](422.md) | A lint at the level `expect` warns when it does not report in a file it is | accepted | 10 |
| [423](423.md) | A lint a dependency defines, `std` aside, reports at most `warn` while a | accepted | 10 |
| [424](424.md) | `#[lang.lint.allow(lint, reason)]` may be written on a local binding as on a | accepted | 10 |
| [425](425.md) | The server's `vun.stop` command ends the programs `vun.run` started that have | accepted | 13 |
| [426](426.md) | A view has a scope | accepted | 13 |
| [427](427.md) | A package may name the target it is written for with `.target` in its | accepted | 12, 13 |
| [428](428.md) | A 32-bit Arm object keeps its relocations naming the symbols and holds the | accepted | 13 |
| [429](429.md) | A `#[layout.c]` field may be a function of `#[abi.naked]` as of `#[abi.c]` | accepted | 5, 7 |
| [430](430.md) | `b.flash(program, .uf2(.{ .family = n }))` is carried out | accepted | 12 |
| [431](431.md) | A `uf2` flash method lists the ways to give its file to a device | accepted | 12 |
| [432](432.md) | A dependency may be `.library = "<path>"`, a package of the toolchain's library | accepted | 12 |
| [433](433.md) | `vun new <path> [--template name]` copies a template, a project of the | accepted | 12 |
| [434](434.md) | `.vun/build/` holds the programs a build writes | accepted | 13 |
| [435](435.md) | A failing tool says whether it is missing | accepted | 12, 13 |
| [436](436.md) | `vun doc --deps` documents the packages a project declares as dependencies, and | accepted | 12 |
| [437](437.md) | A human report shows a source file's path below the working directory when it | accepted | 10 |
| [438](438.md) | The help of `int.does_not_fit` for a constant says to use a value within the | accepted | 04, 10 |
| [439](439.md) | The RP2350 board's default trap records the trap site in the watchdog scratch | accepted | 08 |
| [440](440.md) | The RP2350 board has the NVIC | accepted | 08 |
| [441](441.md) | Verifying ahead keeps what it finds of a memo reaching, through its own reads | accepted | 13 |
| [442](442.md) | `.requires` in a manifest names the oldest toolchain that reads and builds the | accepted | 12 |
| [443](443.md) | `..` as a whole value | accepted | 01 |
| [444](444.md) | A first word of `vun test`, `bench` and `fuzz` that names nothing on disk is | accepted | 11 |
| [445](445.md) | `vun build` and `vun test` of a package, a workspace or a directory check every | accepted | 12 |
| [446](446.md) | A `$for` steps through the items of a tuple, `.{ 1, 2, 3 }` or `.{ u8, u16 }` | accepted | 03 |
| [447](447.md) | A `$if` used as a value reports a trap its condition fires once, at the `$` | accepted | 03 |
| [448](448.md) | A `$` argument that reads a local its body computes when it runs is passed as | accepted | 03, 13 |
| [449](449.md) | A stop on a board with no console is passed on in the reboot into BOOTSEL | accepted | 12 |
| [450](450.md) | The RP2350 board serves a polled USB serial console on the chip's own USB | accepted | 12 |
| [451](451.md) | `rp2350.flash.uf2` delivers by `picotool load -f -x` | accepted | 12 |
| [470](470.md) | A member of the witness an impl declares that leaves its annotation out binds | accepted |  |
| [471](471.md) | An operator whose left operand never finishes | accepted |  |
| [472](472.md) | A `$for` over a value that has no items to step through | accepted |  |
| [473](473.md) | `lang.check.Kind.stack`, a check of each function's frame against the stack | accepted |  |
| [474](474.md) | `lang.arch.wasm32` is an instruction set of a stack machine | accepted | 07 |
| [475](475.md) | `install.sh` runs nothing it has not checked | accepted | 12 |
| [476](476.md) | A `_` array length in the type of a parameter is `types.hole_length`, except | accepted | 12 |
| [477](477.md) | `std.fmt.println` is `print` and a line break | accepted | 03, 05 |
| [478](478.md) | An argument written `$c` for a `Code(T)` parameter, where `c` is code, is that | accepted | 03 |
| [479](479.md) | An annotation that is neither a type nor an interface | accepted | 02 |
| [480](480.md) | Scalable vectors, first as a library over the target's width | accepted | 04, 13 |
| [481](481.md) | `std` gains `std.rand` | accepted | 12 |
| [482](482.md) | A temporary is stored as the type the context expects of it whenever a borrow | accepted | 02 |
| [483](483.md) | Each test line and the totals carry `nanos`, read from the monotonic clock | accepted | 11 |
| [484](484.md) | `std.collections` gains `HashSet(T)` | accepted | 05 |
| [485](485.md) | `lang.mem.Uninit(T)` and `lang.mem.uninit(T)` | accepted | 05 |
| [486](486.md) | An operand passed as asked that is a temporary is stored as the type expected | accepted | 02 |
| [487](487.md) | `vun doc` writes only into a directory that is absent, empty or marked by a | accepted | 12, 13 |
| [488](488.md) | `std.math` | accepted | 08 |
| [489](489.md) | Fixes found building 488 | accepted | 02, 06, 13 |
| [490](490.md) | `std.fmt.float` writes a float as the shortest decimal that reads back as it | accepted | 03, 11 |
| [491](491.md) | `std.os.entropy(bytes)` fills bytes from the system's source for secrets, a | accepted | 08 |
| [492](492.md) | Fixes found building 490 and 491 | accepted | 02, 06, 13 |
| [493](493.md) | `std.fmt.json` writes a float as a JSON number of its shortest digits | accepted | 11, 12 |
| [494](494.md) | `vun test` runs tests in parallel as nextest does | accepted | 11, 13 |
| [495](495.md) | `#[comptime.deferred]` | accepted | 03 |
| [496](496.md) | Language features are `lang.gate.Feature` variants | accepted | 03, 12, 13 |
| [497](497.md) | The Cranelift backend | accepted |  |
| [498](498.md) | The parts of the toolchain are components | accepted |  |
| [499](499.md) | The distributed `vun` has no size budget | accepted | 13 |
| [500](500.md) | A run the server starts can be given input | accepted | 13 |
| [501](501.md) | `lang.arch.wasm32` declares the atomic instructions of the threads proposal | accepted | 07, 13 |
| [502](502.md) | A backend that writes no block of inline assembly in the function holding it | accepted | 13 |
| [503](503.md) | The Cranelift backend lowers C records by value itself, from `vun_abi::Passing` | accepted |  |
| [504](504.md) | `lang.arch.wasm32` declares structured control | accepted |  |
| [505](505.md) | A float constant of a wasm32 block is its bits | accepted |  |
| [506](506.md) | `lang.arch.wasm32` names functions and linker globals in blocks | accepted |  |
| [507](507.md) | `lang.arch.wasm32` declares the 236 vector instructions of the SIMD proposal | accepted |  |
| [508](508.md) | What keeps a function whole under `Outline` is a register that is the caller's | accepted | 13 |
| [509](509.md) | `lang.arch.wasm32.call_indirect(f)` | accepted | 07 |
| [510](510.md) | A Vun vector of 16 bytes binds to a block of `lang.arch.wasm32` as a value | accepted | 07, 13 |
| [511](511.md) | The Cranelift backend places C records by value where the classifier's | accepted | 13 |
| [512](512.md) | The `asm` checks apply to blocks of `lang.arch.wasm32` | accepted | 07, 13 |
| [513](513.md) | `--diagnostics=sarif` is the renderer component `sarif` | accepted | 10 |
| [514](514.md) | A link symbol is a callable like a function and a primitive | accepted | 07, 13 |
| [515](515.md) | `DiagId` is typed | accepted | 10 Ids are equal, hashed and ordered by path text, so `DiagId::new` and `DiagId::from` of one path are one id. |
| [516](516.md) | A loan nested in an aggregate argument of a compile-time call | accepted | 03 |
| [517](517.md) | A compile-time call costs memory by nesting level | accepted | 03, 13 |
| [518](518.md) | `std.Dyn(I)` | accepted | 02, 03 |
| [519](519.md) | Reflection of requirements and function types | accepted | 03, 02 |
| [520](520.md) | `lang.mem.function_cast(F, G, f)` | accepted | 05 |
| [540](540.md) | `Iter.sum(A)` adds the items of a frame into a total of the type `A` the caller | accepted | 04, 06 |
| [541](541.md) | `Iter.peekable()` gives `Peekable(T, F)`, a linear value holding the frame and | accepted | 06 |
| [542](542.md) | `Vec.split_off(A, a, at)` moves the items from `at` on into a new vector | accepted | 05, 06 |
| [543](543.md) | `Iter.sum_floats(A)` adds the items of an iterator into a total of the float | accepted |  |
| [560](560.md) | `std.collections.BTreeMap(K, V)` | accepted | 05 |
| [561](561.md) | `BTreeMap` gives its keys in a range | accepted |  |
| [580](580.md) | `vun cover` | accepted | 11 |
| [600](600.md) | `vun bench` takes samples | accepted | 11 |
| [601](601.md) | `--measure instructions` counts the instructions a process retires in user mode | accepted | 11 |
| [602](602.md) | Baselines | accepted | 11 |
| [620](620.md) | The `vun-sema` query cycle is split by interfaces sema owns and the driver | accepted | 13 |
| [640](640.md) | `#[test.traps()]` | accepted | 11 |
| [641](641.md) | `lang.bounds.last(T, b)` | accepted | 04, 06 |
| [642](642.md) | `Int(r).sign_mask()` is 0, or the value with every bit set | accepted | 04, 11 |
| [645](645.md) | Coverage counts | accepted | 11 |
| [646](646.md) | `std.Dyn(I)` checks the requirements of `I` when its table is built and reports | accepted | 02 |
| [647](647.md) | A loan in a compile-time call's result is rebased on the caller's at any depth | accepted | 03, 13 |
| [648](648.md) | `vun test|bench|fuzz|cover --filter text` runs the tests whose name contains | accepted | 01, 03, 10, 11 |
| [649](649.md) | A lint sees the types typing finds | accepted |  |
| [650](650.md) | Guaranteed tail calls | accepted |  |
| [651](651.md) | Tail calls reach every backend but Cranelift and QBE | accepted |  |
| [652](652.md) | A tail call reuses what its caller received instead of needing registers alone | accepted | 09, 13 |
| [653](653.md) | Tail calls reach Cranelift, and results that the callee writes through the | accepted | 09, 13 |
| [654](654.md) | Backends agree on a program when their runs have one outcome, not one process | accepted | 11, 13 |
| [655](655.md) | A function whose result annotation is an interface but whose body never yields | accepted | 06 |
| [656](656.md) | `witnesses` finds the impls and derived interfaces that cover a type | accepted | 02, 13 |
| [657](657.md) | `cache` and `ffi` are groups of `util` | accepted | 12 |
| [658](658.md) | Commands show progress | proposed | 12 |
| [659](659.md) | Components are a noun of their own, `vun component list\|add\|remove`, not a | accepted | 12 |
| [660](660.md) | A path a command is given that names nothing is `cli.missing_path`, reported | accepted | 11, 12 |
| [661](661.md) | Help pages list an option set by its detail | accepted | 12, 13 |
| [662](662.md) | The diagnostics are declared one file per group | accepted | 10, 13 |
| [663](663.md) | A tuple position is a `u32` | accepted |  |
| [664](664.md) | `vun debug profile` measures the compiler and the programs it builds with the | accepted |  |
| [665](665.md) | BLAKE3 is built with its SIMD code | accepted |  |
| [666](666.md) | Checking a program lints the files of its own package, not those of the | accepted |  |
| [667](667.md) | A toolchain ships the libraries' memos | accepted |  |
| [668](668.md) | Saved memos hold wherever the toolchain and the libraries are | accepted |  |
| [669](669.md) | Machine code is kept per function by content | accepted |  |
| [670](670.md) | The distributed `vun` is optimized by a profile of its own workloads | accepted |  |
| [671](671.md) | Durability is per group of inputs | accepted | 13 |
| [672](672.md) | `vun new` renames what a template names after its package | accepted | 12 |
| [673](673.md) | `#[link.library(name)]` is read by every hosted linker | accepted | 07, 13 |
| [674](674.md) | `c"text"` is a string for C | accepted | 01, 07 |
| [675](675.md) | The binder binds what a request names | accepted | 07 |
| [676](676.md) | A runtime exports the heap and the environment as it does the library loader | accepted | 08 |
| [677](677.md) | `vun new` refuses a directory whose name is no name in code | accepted | 12, 10 |
| [678](678.md) | A package's `.system` lists the libraries of the system its programs are linked | accepted | 07, 12 |
| [679](679.md) | A variant of an enum names its tag, `name = code` | accepted | 05 |
| [680](680.md) | The binder binds a C enum as an integer alias and a constant per value, which | accepted | 05, 07 |
| [681](681.md) | The binder reads the attributes written on a type, units being parsed with | accepted | 07 |
| [682](682.md) | `vun util ffi bind --split --out <file>`, the file inside a package, binds the | accepted | 07, 12 |
| [683](683.md) | A runtime without a C library has a heap | accepted | 08, 05 |
| [684](684.md) | The command line has a closed vocabulary of verbs | proposed | 12 |
| [685](685.md) | `vun check` reports wide intermediates and generic code size on request | proposed | 03, 04, 09, 13 |
| [686](686.md) | A link symbol with a pack is imported as a variadic C function | proposed | 07, 13 |
