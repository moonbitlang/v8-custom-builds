# Custom wee8

This directory owns the custom runtime. `build.sh` copies it unchanged into the
pinned source tree before GN runs. There is one build switch, `v8_custom_wee8`;
the native context and implementation selections are always enabled together.
Ordinary V8 defaults to `false` and keeps its original behavior.

| File or directory | Responsibility |
| --- | --- |
| `BUILD.gn` | Native toolchain scope and capability settings |
| `src/wasm-context-bootstrap.inc` | Private native context initialization |
| `src/cpp-builtin-selection.h` | C++ builtin entries |
| `src/runtime-selection.h` | Runtime helpers and inline aliases |
| `src/generated-js-builtin-selection.h` | Generated JS-entry bodies (TFJ) |
| `tests` | Native probes, reviewed inventories, Wasm guests and one check runner |

The source files are included through the upstream insertion points in
`wasm-native-context.patch`. GN regenerates the embedded builtin blob and matching
snapshot together. The producer does not rewrite archives after compilation.

## Retained implementations

The native context creates function/object maps for callbacks, private array and
buffer maps for results, stacks and Wasm memory, a WeakMap map for host-info
finalization, errors/CallSite diagnostics, factory caches and serializer state.
It installs no JS library globals. Unallocated optional context slots remain
undefined and unsupported.

| Selection | Retained | Rationale |
| --- | ---: | --- |
| C++ builtins | 33 of 488 | Native function-template entry, core sentinels and Error/CallSite diagnostics |
| Canonical runtimes | 144 of 619 | Declared Wasm helpers; shared allocation, interrupts, errors, primitive conversion, diagnostics and host-info ownership |
| Generated JS entries (TFJ) | 120 of 432 | Native entry/coercion, primitives, conservative sequence/Math/DataView fallbacks and Wasm wrappers/imported strings |

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
Retained sequence/Math/DataView bodies are conservative allowances for shared Wasm
machinery, not usable public JS APIs. `WasmJs::PrepareForSnapshot` and Wasm
registration still use upstream machinery. Other generated kinds
(TFC/TFS/TFH/ASM/BCH) retain their bodies. Source exclusion, bytecode-handler removal
and a smaller Wasm registration layer remain separate work.

## Upstream patches

The recipe applies patches in filename order. The files retain separate purposes;
none is an additional runtime selection layer.

| Patch | Purpose |
| --- | --- |
| `host-tools.patch` | Compatibility with supported build runners and toolchains |
| `native-target.patch` | Reuse the native bootstrap and expose artifact/check targets |
| `no-js-optimizer.patch` | Make JS optimizing-tier flags read-only false; retain the shared Wasm compiler |
| `no-regexp.patch` | Optional RegExp capability across Torque, registrations, external references and direct callers |
| `no-source-parser.patch` | Optional parser capability; exclude parser/scanner sources and reject source compilation |
| `wasm-c-api.patch` | Adapt the pinned C API implementation and native link dependencies |
| `wasm-exceptions.patch` | Implement native tags and catchable exception extensions |
| `wasm-native-context.patch` | Connect the context and three implementation selections to V8 |
| `wee8-profile.patch` | Declare the single custom switch, common define and owned headers |
| `wee8-without-asmjs.patch` | Optional asm.js capability; exclude the translator and guard direct callers |

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
| `wee8-runtime-check` | Snapshot initialization, empty globals, script/module/function/streaming rejection, direct asm.js rejection and RegExp API rejection |
| Catalogs vs inventories | Every CPP/TFJ selection and descriptor signature; canonical runtime argument/result metadata |
| `cpp-entry-contract` | Actual CPP metadata and external-reference slots agree and retain distinct entry addresses |
| `runtime-entry-contract` | Actual canonical/inline tables, distinct addresses and pair-return conventions |
| `generated-builtin-contract` | Deserialized builtin identities, code kind and argument count; every omitted TFJ body and CPP adaptor traps |
| `runtime-boundaries` guests | Table operations, bulk memory, guest exceptions, GC roots across native collection, allocation pressure, interrupts, serialization and shared-memory atomics/growth |

Unsupported invocations run in isolated processes and must reach the named entry
or generated trap. Null dereferences, ordinary returns and unrelated failures do
not pass. Context and guest checks run in Liftoff-only, optimizing-only, lazy and
synchronous-tier-up modes. The packaged C API harness reassembles the WAT fixtures
with its existing pinned dependency and checks the binaries match.

IDs and machine-code sizes are architecture-dependent; inventories do not freeze
them. Probes validate IDs against each target's enums and deserialized Code.
Packaged C API/Rust tests validate the consumer interface, including exceptions;
Moonrun validation uses the wee8 backend in `moonbitlang/moon/crates/moonrun`.

## V8 upgrades

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
