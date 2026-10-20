# Wasm context allowlist prototype

The existing C API creates a full V8 context even though its guest is Wasm.
This experiment gives the same adapter a separate bootstrap that allocates
explicitly listed runtime objects and exposes an empty global object. It does
not run `InitializeGlobal`, the iterator initializers, experimental JS-library
installation, or the miscellaneous library installer.

The prototype executes native Wasm calls, callbacks, traps, memory growth, and
WasmGC without initializing the public JS standard library. This establishes a
useful bootstrap seam. The bootstrap alone does **not** establish the minimum
possible runtime or remove implementations rooted by builtin tables.
The subsequent [C++ entry allowlist](CPP-ENTRIES.md) and
[runtime entry allowlist](RUNTIME-ENTRIES.md) remove unused implementation
references while preserving identities, bringing the C API probe to
**12.20 MB**.

## Draft implementation

`wasm-context-bootstrap.inc` contains the allocation allowlist.
`bootstrap.patch` adds its declaration and selects it at the existing bootstrap
call site under `V8_WASM_CONTEXT_PROTOTYPE`. The normal bootstrap implementation
is preserved. The build script applies the same edits to an isolated source
copy, compiles one replacement object, relinks a separate snapshot generator,
regenerates the snapshot and embedded blob, and constructs a separate archive.
No existing build outputs are overwritten.

This prototype define is not yet a supported GN option. For production, select
the module through an explicit option on the existing custom target, defaulting
off for normal V8. Do not infer the mode from whether a JS parser is enabled.

### Allocation allowlist and dependency rationale

| Retained allocation | Why it is present |
| --- | --- |
| Native context, object/function maps and prototypes | V8 factories, native calls, exported function objects and instance exports use them. Constructors are private; no Object or Function global is installed. |
| Function, catch and debug-evaluate context maps; template/map caches | Shared call, object-allocation and inspection infrastructure. This is provisional support, not a claim that the debugger is fully validated. |
| Internal array maps and length descriptor | Factories and captured stack data allocate arrays. Array methods and the Array global are absent. |
| ArrayBuffer and SharedArrayBuffer allocation maps | Wasm memories own backing-store objects. ArrayBuffer methods and globals are absent. Shared memory still requires additional validation. |
| WeakMap allocation map | The unchanged C API uses `Factory::NewJSWeakMap()` for host metadata. Its library methods and global are absent. |
| Error, TypeError, RangeError, SyntaxError and Wasm error layouts | Traps, invalid inputs, compilation/linking errors and clean rejection of disabled JS compilation. The existing error/CallSite helpers still install some diagnostic methods internally. |
| Embedder data, empty extras binding and Wasm debug maps | Existing native-context consumers expect these containers. No console or tracing methods are installed. |
| Random-state cache | `ContextSerializer::Serialize` unconditionally calls `MathRandom::ResetContext`. Retained state satisfies that contract; no Math object or methods are installed. |
| Wasm object constructors/maps | Existing `WasmJs::PrepareForSnapshot` supplies maps consumed by `wasm-objects.cc`. This is a remaining broad dependency: it also constructs JS-facing methods internally. `WasmJs::Install` is called with global exposure disabled. |

Unlisted optional context slots stay undefined. The new initializer does not
walk the full JS initializer and delete unwanted objects afterward. Its private
allocation constructors may use `Illegal` as an unreachable call target; this
does not replace, renumber, or alias entries in the builtin table.

## Measurements

See [Native ThinLTO and upstream size comparison](LTO.md) for the subsequent
measurement against moonrun's upstream rusty_v8 dependency and the effect of
recompiling the native archive with ThinLTO.

Matched ARM64 macOS stripped probe executables, with the same compiler/linker
flags and dead stripping, without ThinLTO:

| Measurement | Baseline | Allowlist | Reduction |
| --- | ---: | ---: | ---: |
| C API probe executable | 13,757,456 B | 13,575,744 B | 181,712 B (1.32%) |
| C API WasmGC probe executable | 13,757,440 B | 13,575,728 B | 181,712 B |
| Initialization-only executable | 13,707,472 B | 13,542,272 B | 165,200 B |
| Isolate used heap after context/rejection test and full-GC request | 462,568 B | 335,528 B | 127,040 B (27.46%) |
| Isolate total heap | 1,884,160 B | 1,884,160 B | 0 B |
| Snapshot payload | 401,698 B | 316,920 B | 84,778 B |
| Embedded builtin blob | identical | identical | 0 B |

Heap readings were identical across three separate processes for each variant.
These are probe executable sizes, **not a measured moonrun release binary**.
They are not an LTO result or a prediction of the final size after table pruning.

Baseline producer revision: `47697b35234e2f4a2f4892b25deb9fcb6c7fed1c`
(includes the separate Promise-closure candidate).
rusty_v8: `e9f20a2493883b53cb58b19a2fe761358dd57996`.
denoland/v8: `245ce17ed8483e6bc3de7f668635f46c5bfdd8a0`, V8 12.9.202.13.
This is not a comparison against a new release.

### Why the executable is still large

The live-symbol link maps show:

| Object | Baseline live symbol bytes | Allowlist live symbol bytes |
| --- | ---: | ---: |
| `bootstrapper.o` | 112,622 | 29,042 |
| `snapshot.o` | 415,227 | 330,449 |
| `embedded.o` | 1,556,160 | 1,556,160 |
| `builtins-date.o` | 20,375 | 20,375 |
| `builtins-temporal.o` | 109,068 | 109,068 |
| `js-temporal-objects.o` | 223,900 | 223,900 |
| `external-reference-table.o` | 19,601 | 19,601 |
| `wasm-js.o` | 52,803 | 52,803 |
| `c-api.o` | 25,676 | 25,676 |

The source and linker reachability trace identify this path, independent of
context creation:

```
V8::Initialize
  -> ExternalReferenceTable::InitializeOncePerIsolateGroup
  -> AddBuiltins::c_builtins
  -> Builtin_TemporalPlainDateConstructor
```

`builtin_metadata` also contains C++ entry pointers. Omitting Temporal from the
context cannot remove these independent roots. Replacing only the C API wrapper
also cannot remove them: its own directly retained code is about 26 KB.

## Validation

The checks below describe the context-only stage. The subsequent
[runtime entry report](RUNTIME-ENTRIES.md#validation) extends coverage to
shared memory, exception tags, runtime identities and the Rust wrapper.

Passed for both context-stage archives:

- Empty, initialization, isolate and context lifecycle probes.
- Native C API import/export calls, callbacks with environment ownership and
  finalization, direct host calls, missing imports, argument/result arity errors,
  runtime traps and trapping start functions.
- Memory reads/writes, successful growth, rejected excessive growth, preservation
  of existing memory and access to newly grown memory.
- WasmGC structs and arrays surviving 100 collections triggered by a native
  import, with correct results after every collection.
- C API reference host-info round-trip and exactly-once finalization at teardown.
- C API and WasmGC probes in Liftoff-only, optimizing-only, lazy compilation and
  synchronous tier-up modes.
- Clean rejection of JavaScript text compilation.
- Candidate global-object own-property list is empty.

The snapshot and embedded blob were regenerated together. The blob is
byte-for-byte identical to the baseline.

No full moonrun integration suite, debugger suite, platform matrix, shared-memory
test, Wasm exception-tag test, externref stress test or production release had
been run at this stage. Public JS object APIs are not a supported interface
for the candidate. Absent native-context slots still need a broader invariant
audit before this can be offered as a supported build.

## Reproduction

Requires a matching completed custom_wee8 build and its archive. Supply paths and
the compiler/archive tools used by that build. Commands use placeholders:

```sh
python3 experiments/wasm-context/build.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" --archive "$BASELINE_ARCHIVE" \
  --work "$EXPERIMENT_OUT" --cxx "$CXX" --ar "$LLVM_AR"
python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" --baseline "$BASELINE_ARCHIVE" \
  --candidate "$EXPERIMENT_OUT/libwee8.a" --work "$EXPERIMENT_CHECKS" --cxx "$CXX"
python3 experiments/wasm-context/analyze-maps.py "$EXPERIMENT_CHECKS"
```

The script is deliberately limited to this ARM64 macOS build layout. It reuses
GN-generated flags rather than maintaining a second compiler configuration.
Source anchors fail when they no longer match; adapting a V8 update requires
reviewing the new initializer and its shared helpers.

## Remaining design work

1. Extract private Wasm object/map setup from `WasmJs::PrepareForSnapshot`.
   Preserve the layouts consumed by native Wasm factories while omitting
   JS-facing constructors and methods. The native adapter should choose this
   context explicitly.
2. Extend the positive [CPP](CPP-ENTRIES.md) and
   [runtime](RUNTIME-ENTRIES.md) entry selections to remaining builtin kinds
   and independent external references after auditing their native Wasm callers.
   Unselected builtin IDs retain their descriptors and explicit unsupported code
   bodies. Keep the snapshot and tables generated from the same selection.
3. Audit runtime tables and transitive references. A builtin's name or JS linkage
   is not a dependency proof; generated Wasm code can call shared helpers.
   Unknown additions on a V8 bump must require classification.
4. Run moonrun's native interface tests and missing feature/error/GC cases against
   the candidate, then measure the final linked moonrun binary under release LTO.

The final size is unknown until the remaining roots are changed and the consumer
is linked. The context-only C API probe measured 13.58 MB, CPP entry selection
12.91 MB, and runtime entry selection 12.20 MB. None is a floor or a target.
