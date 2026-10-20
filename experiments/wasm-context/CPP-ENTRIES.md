# C++ builtin entry allowlist

Selecting the native C++ builtin entries saves another **662,304 bytes (4.88%)**
in the C API probe, reducing it from **13.58 MB to 12.91 MB** without native LTO.
This follows the private Wasm-context bootstrap and addresses the two
function-pointer tables that kept unused library implementations alive.

The common context probe is now **12,880,160 bytes**, down from 24,043,920 bytes
with moonrun's upstream rusty_v8 0.102.0: **46.43% smaller**. Compared with
upstream 0.106.0 at the same V8 version, it is **45.15% smaller**.
The [upstream comparison's configuration limits](LTO.md#common-context-probe)
still apply. These are probe sizes, not a measured full moonrun application.

## Selection and invariants

[`cpp-builtin-selection.h`](cpp-builtin-selection.h) explicitly retains **33**
of the target's **488** CPP builtins. The remaining **455** receive distinct,
named unsupported native entries. This is a positive selection; there is no
name-prefix rule or blanket exclusion of JS-linkage builtins.

The same selection controls three consumers:

1. `builtin_metadata` selects the C++ entry address.
2. `ExternalReferenceTable::AddBuiltins` selects the same address in the original
   table position.
3. `SetupBuiltinsInternal` emits the original adaptor for retained entries and
   an explicit trap body for omitted entries.

Each native trap is a template instantiation parameterized by its builtin ID.
It terminates with the omitted builtin's name. Each generated builtin retains
its own code object and ID. Builtin counts, descriptors, registration order,
external-reference slots and object layouts are preserved. Neither table is
shortened, and omitted entries are not aliases of `Illegal`.

Both the embedded blob and snapshot are regenerated with this selection. Their
contents change; their sizes happen to remain unchanged in this build.

The original implementation bodies stay in their source files. The guarded
address selection returns the original function when
`V8_WASM_CONTEXT_CPP_ALLOWLIST` is absent. This experiment uses copied source
files, replaces three snapshot-generator objects and two runtime table objects,
and builds a separate archive. It does not change parser removal, shared
compilers, runtime tables or native Wasm wrappers.

## Dependency rationale

The complete per-entry selection is [`cpp-builtins.tsv`](cpp-builtins.tsv).
The catalog is produced by expanding V8's actual `BUILTIN_LIST_C` with the
target definitions, using [`cpp-builtin-catalog.cc`](cpp-builtin-catalog.cc).
The build refuses a catalog mismatch, so adding or renaming builtins requires
review on a V8 update.

| Selection | Rationale |
| --- | --- |
| Three native API entries | Preserve function-template construction and call delegates shared with native adapters. |
| Five core entries | Preserve empty functions, illegal invocation handling, poison-pill errors and unsupported-call diagnostics. |
| Three Error entries and 22 CallSite entries | Preserve error construction, formatting and stack machinery used by Wasm traps and compile/link diagnostics. CallSite entries are conservatively retained as a group; debugger completeness is not established. |
| Omit Date, Temporal, JSON, URI and console CPP entries | The private context does not install these APIs. Their table addresses independently retained implementations even when no context operation used them. |
| Omit Array/ArrayBuffer/SharedArrayBuffer/DataView/typed-array library CPP entries | The native Wasm paths allocate and manage backing stores through factories and memory objects, not these JS constructors or methods. Their required maps remain in the context. Memory growth and preservation tests pass. Shared Wasm memory still needs dedicated validation. |
| Omit String/Number/BigInt/Symbol library CPP entries | This omits JS constructors and methods, not primitive representations, conversion runtimes or generated Wasm string/numeric helpers. Those are outside this CPP-entry selection. This does not claim complete removal of string support. |
| Omit other library CPP entries | Collection methods, JS function construction, reflection, disposable stacks, shared JS objects and JS Atomics APIs have no supported interface in the private context. Wasm instructions and native runtime operations remain separate. |
| Intl | Intl is already disabled in this build. This experiment provides no additional Intl saving or enabled-Intl validation. |
| Promise and other generated builtin kinds | Their previous generated-body selection is unchanged. This step selects CPP entries only; it is not a complete builtin allowlist. |

Passing native tests does not prove all transitive dependencies. Unknown native
calls now fail with an identifiable unsupported entry, instead of silently
returning fabricated results. Direct references from retained runtime functions
can still retain implementations independently of these tables.

## Measurements

Matched ARM64 macOS, LLVM 18 linker, dead stripping, `strip -x`, no native LTO.
MB uses decimal bytes. The baseline is the context-only prototype measured in
[`LTO.md`](LTO.md), not upstream full V8.

| Probe or payload | Context only | Plus CPP selection | Reduction |
| --- | ---: | ---: | ---: |
| Context executable | 13,542,480 | 12,880,160 | 662,320 |
| C API executable | 13,575,888 | 12,913,584 | 662,304 |
| C API WasmGC executable | 13,575,888 | 12,913,568 | 662,320 |
| Snapshot payload | 316,920 | 316,920 | 0 |
| Embedded object live symbol bytes | 1,556,160 | 1,556,160 | 0 |

The native archive still contains the original object files. These savings are
in the linked executable; source exclusion and archive packaging are separate.
No extra LTO rebuild was needed.

The C API link map confirms actual implementation removal:

| Object | Before live symbol bytes | After |
| --- | ---: | ---: |
| `builtins-temporal.o` | 107,674 | 0 |
| `temporal-parser.o` | 30,764 | 0 |
| `js-temporal-objects.o` | 225,131 | 1,056 |
| `builtins-date.o` | 20,375 | 0 |
| `dateparser.o` | 1,288 | 0 |
| `date.o` | 19,413 | 3,908 |
| `json-stringifier.o` | 54,422 | 0 |
| `json-parser.o` | 68,256 | 66,276 |
| `builtins-arraybuffer.o` | 7,470 | 0 |
| `builtins-string.o` | 16,112 | 0 |

These are symbol-attribution sizes, not independent executable savings that can
be summed without accounting for layout and shared code. The unique native
traps add approximately 16 KB to `builtins.o`.

The remaining Date clock/cache code, Temporal runtime support and JSON parser
show why this does not remove every implementation in those families.
The next dependency boundary is the runtime/external-reference registrations,
followed by the remaining generated builtin bodies. Classify their native Wasm
callers before applying the same positive-selection approach.

## Validation

Passed against the candidate archive:

- Context startup, JS source rejection and empty global-object property list.
- C API calls, native callbacks and ownership, traps, memory access/growth,
  invalid imports and arities.
- WasmGC structs and arrays across 100 native-callback collections.
- Both C API probes in Liftoff-only, optimizing-only, lazy compilation and
  synchronous tier-up modes.
- Rust wee8 wrapper: 20 unit/integration tests and four doctests, including
  externrefs across callbacks/GC, store isolation, serialization, SIMD,
  saturating conversions, start functions and recovery after traps.
- All **2,169 deserialized builtin code identities** match their IDs.
- All **488 CPP entry addresses are distinct**, non-null and match the named
  external-reference slot; all **455 omitted addresses** resolve to their
  corresponding unsupported function.
- Deliberate calls to omitted Date, Temporal, JSON, ArrayBuffer and String
  entries terminate with the correct builtin name.

The exact measurements and contract output are in
[`cpp-entry-results.json`](cpp-entry-results.json).
This remains an experiment on the prototype branch. Full moonrun integration,
shared memory, Wasm exception tags, debugging and the platform matrix remain
unvalidated. Production enablement needs an explicit custom-target GN option
and matching snapshot/table configuration on every supported platform.

## Reproduction

Start with the context-only `build.py` output and the matching custom_wee8 GN
build. The scripts rebuild only affected objects and regenerate the snapshot.

```sh
python3 experiments/wasm-context/build-entry-allowlists.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --context "$EXPERIMENT_OUT" --work "$CPP_OUT" \
  --cxx "$CXX" --ar "$LLVM_AR"

python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --baseline "$EXPERIMENT_OUT/libwee8.a" --candidate "$CPP_OUT/libwee8.a" \
  --work "$CPP_CHECKS" --cxx "$CXX" --linker "$LLVM_LINKER" \
  --probes context c-api c-api-gc --cpp-entry-contract

WEE8_ARCHIVE="$CPP_OUT/libwee8.a" \
WEE8_HEADER="$RUSTY_V8_SOURCE/v8/third_party/wasm-api/wasm.h" \
cargo test --locked --manifest-path crates/wee8/Cargo.toml
```

[`cpp-entry-selection.patch`](cpp-entry-selection.patch) records the three V8
source edits. The new header is supplied by the experiment include path. This
patch is recorded for review; it is not part of the production patch stack.
