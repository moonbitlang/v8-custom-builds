# Custom wee8

This directory owns the custom runtime. `build.sh` copies it unchanged into the
pinned source tree before GN runs. There is one build switch, `v8_custom_wee8`;
the native context and implementation selections are always enabled together.
Ordinary V8 defaults to `false` and keeps its original behavior.

| File or directory | Responsibility |
| --- | --- |
| `BUILD.gn` | Native toolchain scope and capability settings |
| `src/c-api.cc`, `src/c-api.h`, `src/wasm.hh` | Owned C adapter and its private C++ declarations |
| `include/wasm.h`, `include/wee8.h` | Standard and additive public C APIs |
| `upstream.json` | Exact baselines and digests for copied upstream files |
| `src/wasm-context-bootstrap.inc` | Private native context initialization |
| `src/wasm-feature-policy.h` | Positive list of optional Wasm features |
| `src/cpp-builtin-selection.h` | C++ builtin entries |
| `src/runtime-selection.h` | Runtime helpers and inline aliases |
| `src/generated-js-builtin-selection.h` | Generated JS-entry bodies (TFJ) |
| `tests` | Native probes, reviewed inventories, Wasm guests and one check runner |

The adapter is selected by a small GN patch; runtime selection files use the
upstream insertion points in `v8/0005-wasm-native-context.patch`. GN regenerates the embedded builtin blob and matching
snapshot together. The producer does not rewrite archives after compilation.

## Retained implementations

The native context creates function/object maps for callbacks, private array and
buffer maps for results, stacks and Wasm memory, a WeakMap map for host-info
finalization, errors/CallSite diagnostics, factory caches and serializer state.
It also creates the private Wasm allocation constructors and exception-package
layout used by V8 factories, without the JavaScript `WebAssembly` namespace or
its prototype methods. Unallocated optional context slots remain undefined and
unsupported.

`wasm-context-bootstrap.inc` owns this initialization path. V8's existing
`mksnapshot` serializes its heap and regenerates the matching builtin blob; each
store restores that context through the ordinary deserializer. The context layout
and snapshot format stay upstream-compatible. After restoration, the initializer
refreshes JSTag's canonical type ID in the running engine, as upstream does.
Snapshot generation and restoration are not reimplemented.

| Selection | Retained | Rationale |
| --- | ---: | --- |
| C++ builtins | 33 of 488 | Native function-template entry, core sentinels and Error/CallSite diagnostics |
| Canonical runtimes | 144 of 619 | Declared Wasm helpers; shared allocation, interrupts, errors, primitive conversion, diagnostics and host-info ownership |
| Generated JS entries (TFJ) | 63 of 432 | Native entry/coercion, primitives, sequence fallbacks and Wasm wrappers/imported strings |

The lists are the selection mechanism. The reviewed `tests/*-inventory.tsv` files
record names, kinds, signatures and decisions so upstream changes require review.
Counts describe the pinned build with Intl and RegExp disabled.

Unselected C++ and runtime entries use distinct typed traps that name the original
entry and terminate. All three C++ entry consumers use the same selection:
builtin metadata, external references and generated adaptors. Runtime inline
aliases keep the canonical entry; pair-return traps keep the original ABI.
Direct references outside these tables can still retain implementations.

Unselected TFJ bodies and C++ adaptors contain explicit traps. The original
builders preserve builtin IDs, descriptors, argument metadata and separate Code
objects. They never alias everything to `Illegal`. Blob offsets and sizes can
change when regenerated; retained metadata does not preserve omitted API behavior.

Date, Promise, Temporal, collection, JSON and other unselected APIs are unsupported.
Retained sequence bodies are conservative allowances for shared Wasm machinery.
Math/DataView JS entrypoints are omitted: C API host imports use V8's Wasm-to-C
call path, not its recognized JavaScript-builtin import path. Wasm numeric
instructions, loads/stores and native JS-string builtins keep their own
implementations. Other generated kinds (TFC/TFS/TFH/ASM/BCH) retain their bodies.
The external-reference table still retains JavaScript Wasm callbacks; pruning
those references and their source is separate work.

## Wasm feature policy

The C API applies `src/wasm-feature-policy.h` before engine initialization. Its
positive list retains modern exception handling (`exnref` in V8 12.9's flags)
and Wasm inlining. All other optional flags are disabled, including the broad
`wasm_staging` switch. Feature-set construction also applies the list, so V8's
origin-trial callbacks cannot re-enable excluded proposals. This matters because
the pinned imported-strings callback also enables legacy stringref upstream.

| Capability | Policy |
| --- | --- |
| Modern `try_table`, `throw_ref`, tags and exceptions | Retained |
| `wasm:js-string` builtins and imported string literals | Explicit per-module compile options, independent of proposal flags |
| Legacy `stringref` types, instructions and string views | Disabled (already off by default upstream) |
| Legacy `try`, `catch`, `catch_all`, `delegate`, `rethrow` opcodes | Disabled (enabled by default upstream); use modern exception handling |
| Memory64, JSPI/type reflection, stack switching, shared-everything GC types, FP16, hints/tracing and unsafe GC experiments | Remain disabled in this pinned runtime |
| Unconditional V8 features, including GC, typed references, SIMD, threads/atomics and multiple memories | Unchanged; host C API restrictions still apply |

Disabled does not mean obsolete: for example, [Memory64 and JS-string builtins
are standardized](https://github.com/WebAssembly/proposals/blob/main/finished-proposals.md).
The [modern exception design](https://github.com/WebAssembly/exception-handling/blob/main/proposals/exception-handling/Exceptions.md)
is distinct from the legacy exception instructions. We do not turn on every
prototype from this older V8 just because its proposal later advanced.

The new string wrapper represents a rooted V8 primitive transported as
`externref`. It does not expose the [old stringref proposal](https://github.com/WebAssembly/stringref).
The [JS-string builtins interface](https://github.com/WebAssembly/js-string-builtins/blob/main/proposals/js-string-builtins/Overview.md)
uses imports instead of adding those string types/instructions to the wire format.

This policy bounds accepted modules; it does not remove compiler implementations
or promise an archive-size reduction. Existing compiled caches are not a portable
interchange format across policy/version changes. On a V8 upgrade, review both
optional flags and features made unconditional upstream.

## Upstream patches

[`patches/series`](../patches/series) lists the explicit application order, grouped
by dependency. Each patch carries its purpose, upgrade obligation and test contract.
See [MAINTENANCE.md](MAINTENANCE.md) for regeneration and adapter review.

| Patch | Purpose |
| --- | --- |
| `rusty_v8/0001-host-tools.patch` | Build-runner and toolchain compatibility |
| `chromium-build/0001-host-tools.patch` | Chromium toolchain compatibility |
| `rusty_v8/0002-native-target.patch` | Reuse the native bootstrap and expose artifact/check targets |
| `v8/0001-no-js-optimizer.patch` | Disable JS optimizing tiers while retaining the shared Wasm compiler |
| `v8/0002-no-regexp.patch` | Optional RegExp capability across generators and direct callers |
| `v8/0003-no-source-parser.patch` | Exclude parser/scanner frontends and reject JS source compilation |
| `v8/0004-wasm-feature-policy.patch` | Constrain feature-set construction to the owned optional-feature policy |
| `v8/0005-wasm-native-context.patch` | Connect the context and implementation selections to V8 |
| `v8/0006-wee8-canonical-type-storage.patch` | Backport rollback of duplicate canonical-type candidates |
| `v8/0007-wee8-profile.patch` | Declare the custom switch, common define and owned headers |
| `v8/0008-wee8-without-asmjs.patch` | Exclude the asm.js frontend and guard direct callers |
| `v8/0009-select-owned-c-api.patch` | Select the owned adapter and preserve native link dependencies |

ICU, Maglev and Sparkplug use existing GN settings. Parser and asm.js source lists
exclude only their frontends; shared AST/Wasm metadata remains. The disabled
parser reports a compilation error, and the retained asm.js entry reports a
TypeError rather than attempting lazy JS compilation. RegExp guards also cover
shared string protocol checks and serialization; shared object layouts and GC
metadata remain. These capability patches preserve upstream enabled behavior.
The former horizontal library omission patch is replaced by the positive lists.

## Validation

Build the producer, then run:

```sh
python3 custom_wee8/tests/check.py target/native-checks/<target>
```

| Check | Contract |
| --- | --- |
| `wee8-runtime-check` | Snapshot restoration without a private JavaScript WebAssembly API, empty globals, script/module/function/streaming rejection, direct asm.js rejection and RegExp API rejection |
| Catalogs vs inventories | Every CPP/TFJ selection and descriptor signature; canonical runtime argument/result metadata |
| `cpp-entry-contract` | Actual CPP metadata and external-reference slots agree and retain distinct entry addresses |
| `runtime-entry-contract` | Actual canonical/inline tables, distinct addresses and pair-return conventions |
| `generated-builtin-contract` | Deserialized builtin identities, code kind and argument count; every omitted TFJ body and CPP adaptor traps |
| `runtime-boundaries` guests | Numeric rounding, signed zero, floating-point loads/stores, table operations, bulk memory, guest exceptions, GC roots across native collection, allocation pressure, interrupts, serialization and shared-memory atomics/growth |
| `store-lifetimes` | Fresh-store teardown releases foreground task runners and grown shared-memory backing storage; repeated deserialization keeps singleton and recursive-group canonical storage bounded |

Unsupported invocations run in isolated processes and must reach the named entry
or generated trap. Null dereferences, ordinary returns and unrelated failures do
not pass. Context and guest checks run in Liftoff-only, optimizing-only, lazy and
synchronous-tier-up modes. The boundary probe attempts to enable every excluded feature and the imported-string
origin-trial callback. Packaged tests reject legacy exception opcodes, old stringref,
Memory64 and shared GC types through both ordinary and JS-string-enabled compilation.
The packaged C API harness reassembles the WAT fixtures
with its existing pinned dependency and checks the binaries match.

IDs and machine-code sizes are architecture-dependent; inventories do not freeze
them. Probes validate IDs against each target's enums and deserialized Code.
Packaged C API/Rust tests validate the consumer interface, including exceptions;
The packaged stack-limit guest checks independent store limits, deeper recursion
with a larger budget, zero rejection, and recovery after restoring the boundary.
The packaged exception-trace guest checks construction without guest frames,
original call sites, copied exception roots, and frame preservation across rethrow.
The packaged externref guest checks i31 zero and signed boundaries, GC structs and
arrays, callbacks and exception payloads across proven collection, globals/tables,
null handling, and module serialization.
Moonrun validation uses the wee8 backend in `moonbitlang/moon/crates/moonrun`.

Lifetime checks keep one engine alive while repeatedly creating and deleting
stores, matching an embedded runtime that reuses compiled modules. They check
foreground task-runner ownership and V8's canonical-type storage after warmup,
not RSS thresholds. Each deserialized guest also executes, including GC types
whose canonical descriptions must remain valid. The native recipe runs these
checks against the produced archive on all five platforms before publication.

For ownership changes, test both returned values and resource lifetime. Include
repeated operations under the longer-lived owner, not just process shutdown;
reachable queues and caches can retain memory even when a leak scanner reports
no unreachable allocations. Use allocator profiles to investigate growth, then
turn the identified ownership or bounded-storage invariant into a deterministic
regression. Passing short functional tests alone does not establish cleanup.

## V8 upgrades

Follow the [maintenance contract](MAINTENANCE.md#upgrading-and-validating), including
reconciliation of the owned adapter with its recorded upstream baseline. In addition:

1. Reapply the insertion patches. Audit private-context allocations against their
   factory/serializer consumers and review direct parser/asm.js/RegExp callers.
   Drop build-host fixes when upstream supplies them.
2. Inspect both GN scopes: only custom wee8 receives the selections and capability
   cuts. This does not require compiling ordinary wee8.
3. Review inventory changes. New CPP/TFJ/runtime entries default to unsupported;
   new members of the declared Wasm runtime group are retained and require review.
4. Run native probes, packaged C API/Rust suites and the actual Moonrun wee8 backend.
   When upgrading its wrapper, compare both archives using the same wrapper so
   wrapper behavior changes are not mistaken for native runtime regressions.
5. Validate all supported platforms before publication. Never mix generated blobs,
   snapshots or headers from different recipes. Further cuts must demonstrate an
   additional size or build-cost benefit.

## Public C headers

`include/wee8.h` owns all wee8 extension declarations and includes the unmodified
upstream `wasm.h` at 9d6b93764ac96cdd9db51081c363e09d2d488b4d. Both live in
`include/`; the owned adapter implements extensions without editing the standard
header. Its private C++ declarations live in `src/` and are not a public interface.
Both headers ship together, while the recipe check pins the upstream bytes and
compiles both include modes as C and C++. The packaged probes include `wee8.h`.
Runtime `wasm_tag_*` functions remain extensions despite their existing prefix;
keeping the names preserves the released native ABI.
