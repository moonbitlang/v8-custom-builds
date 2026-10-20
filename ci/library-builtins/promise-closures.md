# Private Promise combinator closures

The five public combinators in the existing profile are generated Torque
builtins. Unlike Date, they have no corresponding C++ implementations to replace.
Their four private element closures still occupied embedded code after the public
entries were trapped. This cut replaces those generated closure bodies using the
existing `SetupBuiltinsInternal` selection.

| Builtin | Only creator | Original instructions | Replacement |
| --- | --- | ---: | ---: |
| PromiseAllResolveElementClosure | Promise.all | 1,084 B | 24 B |
| PromiseAllSettledResolveElementClosure | Promise.allSettled | 1,456 B | 24 B |
| PromiseAllSettledRejectElementClosure | Promise.allSettled | 1,456 B | 24 B |
| PromiseAnyRejectElementClosure | Promise.any | 1,152 B | 24 B |

The explicit list is `MOONRUN_OMITTED_PROMISE_CLOSURES`. The allocation sites are
in `promise-all.tq` and `promise-any.tq`, reached only through the already-omitted
combinators. `promise-all-element-closure.tq` supplies the all/allSettled bodies.
`setup-heap-internal.cc` still creates all four shared-function roots. Async
stack inspection in `isolate.cc` recognizes their IDs but does not create or
invoke them. Those identities, roots, descriptors and context layouts remain.

Promise constructors, resolve/reject functions, thenable assimilation,
then/catch/finally, reaction jobs and microtasks remain. Wasm's asynchronous
compilation and Promise integration require that machinery. Removing Promise
as a type would be a separate runtime contract change, not this cut.

Original generator implementations remain intact. The selection stays inside
the existing custom body-profile guard, preserving ordinary V8 behavior.
Both the embedded blob and snapshot are regenerated.

## Validation and measurements

The probe checks all 149 existing public registrations and the four real
shared-function roots, distinct Code identities, argument metadata and traps.
It invokes candidate closures without combinator state, requiring the explicit
trap before any context read. Control closures are inspected but not invoked.
Retained fulfillment and rejection handlers run through the microtask queue,
alongside Wasm and iterator tests in four compilation modes.

For a matched incremental control, retain the new manifest and probe while
using the previous `setup-builtins-internal.cc` selection. Then compare:

```sh
python3 ci/library-builtins/check.py --before BEFORE/probe \
  --before-promise-cut --after AFTER/probe --output identities.json
python3 ci/library-builtins/measure.py --before BEFORE --after AFTER \
  --identities identities.json --output sizes.json
```

The original 149 bodies must remain the same size; only the four closures may
shrink. The baseline archive matches the pre-follow-up body profile. All boundary,
retained-protocol, C API and Rust adapter tests passed for the measured artifacts.
Native CI must validate the PR candidate independently.

| Component | Before | After | Saved |
| --- | ---: | ---: | ---: |
| Embedded code | 1,469,888 B | 1,464,800 B | 5,088 B |
| Embedded metadata | 81,684 B | 81,588 B | 96 B |
| Snapshot payload | 402,096 B | 401,698 B | 398 B |
| Static archive | 60,484,120 B | 60,483,632 B | 488 B |
| Stripped test consumer | 15,359,312 B | 15,359,312 B | 0 B |

These ARM64 macOS measurements do not enable V8 ThinLTO. Generated payload
reductions do not translate directly into linked-file savings: the measured
executable's segment sizes are unchanged. This is a small completion of the
combinator cut, not a substantial standalone executable-size improvement.

## Other optional subsystems

- Intl already uses `v8_enable_i18n_support=false`; its implementation sources
  and ICU dependency are excluded by the upstream build feature.
- RegExp's parser, compiler, interpreter, machine-code backend and entry bodies
  compile to zero text in the custom profile. Some object-layout, predicate and
  utility support remains; removing that is distinct from disabling matching.
- Temporal is only runtime-disabled. Its builtin, object implementation and
  parser objects still contain 109,109, 233,851 and 30,764 bytes of text,
  respectively. This is an audit of compilation units, not an executable-saving
  measurement. A separate build feature should exclude those implementations,
  reject attempts to enable Temporal, and preserve required layout metadata.
  Date's `toTemporalInstant` bridge and the Temporal runtime entry also need
  explicit treatment; disabling registration alone is insufficient.

The retained runtime needs more than scalar types: objects, arrays/buffers,
typed arrays, iteration, Promise reactions, exceptions and GC support serve
Wasm and the embedding API. Remove optional APIs by dependency, rather than
removing every non-scalar type.
