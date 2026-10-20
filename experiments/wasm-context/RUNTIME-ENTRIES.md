# Runtime helper allowlist

Extending the positive selection to V8's runtime registry saves another
**714,320 bytes (5.53%)** in the C API probe: **12.91 MB → 12.20 MB**, without
native LTO. It builds on the private context and C++ builtin selections.

The common context probe is **12,182,336 bytes**. That is **49.33% smaller**
than the same probe with moonrun's upstream rusty_v8 0.102.0, or **48.12%
smaller** than upstream 0.106.0 at the custom build's V8 version.
The [upstream build-profile differences](LTO.md#common-context-probe) still
apply. This is not a size measurement of a complete custom moonrun application.

## One registry supplies both tables

The new source boundary is `kIntrinsicFunctions` in `runtime.cc`.
[`runtime-selection.h`](runtime-selection.h) selects its implementation
addresses. `ExternalReferenceTable::AddRuntimeFunctions` already obtains its
addresses through `ExternalReference::Create(id)` and
`Runtime::FunctionForId(id)`, so it automatically uses the same selection.
There is no second independently maintained external-reference list.

The profile retains **144 of 619 runtime implementations**. Each of the other
**475** gets its own named unsupported trap. All IDs, names, argument counts,
result counts and external-reference positions remain unchanged. The **19
inline aliases** retain their IDs and share their canonical runtime's fallback
address. The **two pair-return entries** use correctly typed `ObjectPair`
trap functions, preserving their calling convention.

The change leaves all implementation source files intact. Without
`V8_WASM_CONTEXT_RUNTIME_ALLOWLIST`, the address macro selects the original
functions. The experiment does not alter parser removal, the Wasm compilers,
native wrappers or the existing generated-builtin selection.

The snapshot and embedded blob are regenerated with the selected table.
The embedded blob is byte-identical to the preceding CPP-selection build:
generated code uses the same external-reference indices.

## Explicit selection

[`runtime-entries.tsv`](runtime-entries.tsv) records every canonical runtime
entry, its argument/result counts and its retained/omitted classification.
The catalog comes from V8's macro expansion under the actual target definitions.
The build rejects any mismatch, requiring review when V8 changes its registry.

| Retained group | Count | Reason |
| --- | ---: | --- |
| V8's native Wasm runtime group | 57 | Compilation, tiering, wrappers, tables, memory, exceptions, GC arrays, debugging and Wasm string helpers. Selection is by the declared group, not a name-prefix heuristic. |
| Shared core helpers | 31 | Allocation and fatal-allocation paths, C/JS entry machinery, interrupts/unwinding, native object and WeakMap operations, shared-memory length and the existing clean asm.js rejection. |
| Shared error helpers | 20 | Preserve native error creation, propagation, stack overflow and invalid-call diagnostics. |
| Primitive helpers | 26 | Preserve BigInt/number conversion and string slow paths used by Wasm/imported-string builtins and native diagnostics. |
| Diagnostic hooks | 10 | Preserve GC requests, stepping/break hooks and Wasm tracing. |

Some shared helpers are conservatively retained because the unchanged native
wrappers and generated builtins still share V8 machinery. This is an audited
initial selection, not a proof that all 144 are individually indispensable.

Unselected entries include JS compilation/eval, class and scope operations,
module import, library-only helpers, JS testing/fuzzing controls and most JS
inline-cache slow paths. For example, `CompileLazy` is omitted while
`WasmCompileLazy` is retained. Wasm tracing entries are retained even though V8
places them in its testing group.

The selection does not claim arbitrary JS execution, Promise integration or
JITless support. No valid supported operation should reach an omitted entry;
if one does, it terminates with the runtime name rather than returning a fake
result. Direct references outside this registry can still retain code.

## Measurements and linker evidence

Matched ARM64 macOS executables, LLVM 18 linker, dead stripping and `strip -x`.
No native LTO. MB means 1,000,000 bytes.
Baseline: prototype revision `567d328`, with context and CPP entry selection.

| Probe | Previous stage | Runtime selection | Reduction |
| --- | ---: | ---: | ---: |
| Context | 12,880,160 | 12,182,336 | 697,824 |
| C API | 12,913,584 | 12,199,264 | 714,320 |
| C API WasmGC | 12,913,568 | 12,199,248 | 714,320 |
| Snapshot payload | 316,920 | 316,920 | 0 |

Representative live symbol bytes from the C API link map:

| Object | Before | After |
| --- | ---: | ---: |
| `random-module-generation.o` | 178,709 | 208 |
| `wasm-module-builder.o` | 36,988 | 0 |
| `ic.o` | 57,665 | 2,404 |
| `liveedit.o` | 32,952 | 752 |
| `debug-coverage.o` | 25,852 | 60 |
| `runtime-test.o` | 27,611 | 603 |
| `runtime-scopes.o` | 15,174 | 0 |
| `runtime-literals.o` | 10,082 | 0 |
| `runtime-compiler.o` | 7,990 | 164 |
| `runtime-temporal.o` | 152 | 0 |
| `js-temporal-objects.o` | 1,056 | 0 |
| `json-parser.o` | 66,276 | 66,276 |
| `date.o` | 3,908 | 3,908 |

The remaining Temporal implementation objects disappear in this probe. JSON
parsing and Date clock/cache support remain rooted elsewhere; this experiment
does not remove clock facilities. Unique native runtime traps add about 17 KB
to `runtime.o`. Symbol attribution is not independently summable executable
savings because alignment and shared code also matter.

The archive retains original object files. The measured saving is in the
linked consumer, not a claim that every omitted implementation was removed
from archive packaging.

## Validation

The candidate passes the prior context, C API and WasmGC tests in Liftoff-only,
optimizing-only, lazy-compilation and synchronous-tier-up modes. The Rust wee8
wrapper again passes **20 unit/integration tests and four doctests**, including
externrefs, serialization, SIMD, start functions and trap recovery.

New native guest probes pass on **both baseline and candidate**, in all four
compilation modes, after serializing/deserializing their modules:

- Table grow/init/copy/fill/get/set, `ref.func`, indirect calls and an
  out-of-bounds table trap.
- Bulk memory init/copy/fill/drop.
- A Wasm tag throw/catch carrying an i32 payload.
- Allocation of 200,000 GC structs, retaining the final value through a global.
- Shared-memory atomic load/store/RMW, zero-timeout 32- and 64-bit waits,
  notification, memory growth with old data preserved and new pages zeroed,
  and an unaligned-atomic-access trap.
- Interrupt delivery requested from a native callback and handled during
  subsequent Wasm execution.

The tests retain shared memory inside the guest. They do not establish
multithreaded wakeup behavior or support for shared-memory import/export in the
Rust wrapper. Exception coverage is one tag/throw/catch case, not the full
exception-handling proposal or JS exception interoperation.

Registry checks verify all 619 canonical entries are distinct, their metadata
and external-reference slots match, all 19 inline aliases preserve their
fallbacks, and both pair-return ABIs are preserved. Six deliberate omitted
runtime calls terminate with the correct diagnostic, including both pair-return
entries. The existing 2,169 builtin-identity and 488 CPP-entry checks also pass.

Exact results are in [`runtime-entry-results.json`](runtime-entry-results.json).
The probes and catalogs are committed alongside this report.

## Reproduction and landing boundary

The driver is now named `build-entry-allowlists.py`; it replaces
`prune-cpp-builtins.py` and still supports CPP-only selection by default.
Start with the context-only output from `build.py`:

```sh
python3 experiments/wasm-context/build-entry-allowlists.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --context "$CONTEXT_OUT" --work "$RUNTIME_OUT" \
  --cxx "$CXX" --ar "$LLVM_AR" --runtime-allowlist

python3 experiments/wasm-context/check.py \
  --source "$RUSTY_V8_SOURCE" --out "$GN_OUT" \
  --baseline "$CPP_OUT/libwee8.a" --candidate "$RUNTIME_OUT/libwee8.a" \
  --work "$RUNTIME_CHECKS" --cxx "$CXX" --linker "$LLVM_LINKER" \
  --probes context c-api c-api-gc --cpp-entry-contract --runtime-checks

WEE8_ARCHIVE="$RUNTIME_OUT/libwee8.a" \
WEE8_HEADER="$RUSTY_V8_SOURCE/v8/third_party/wasm-api/wasm.h" \
cargo test --locked --manifest-path crates/wee8/Cargo.toml
```

Boundary guests require `wasm-tools parse`; `--wasm-tools` can select its path.
[`runtime-entry-selection.patch`](runtime-entry-selection.patch) records the
additional V8 source change.

For landing, the coherent unit is one opt-in custom Wasm profile that selects
the private context, CPP entries and runtime entries together, and regenerates
its matching snapshot. The experimental independent switches are useful for
measurements but should not expose unsupported production combinations.
Keep the normal V8 path unchanged and make registry drift fail validation.

Before enabling that profile for a release, integrate the producer GN target,
exercise the actual moonrun consumer and run the supported-platform matrix.
This turn adds no production GN switch, release, consumer pin or CI matrix.
Remaining generated builtin bodies and independent external-reference roots
are separate follow-up boundaries, not implicitly covered by this whitelist.
