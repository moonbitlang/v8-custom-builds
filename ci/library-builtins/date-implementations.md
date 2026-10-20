# Remove unsupported Date C++ implementations

This follow-up removes the original C++ implementations behind 25 Date entry
points already rejected by the custom wee8 generated-body profile. It does not
add unsupported APIs: the same 149 JS entries still trap.

`v8_wee8_omit_date_method_implementations` defaults to the custom wee8 marker.
The dedicated `v8_date_builtins` source target scopes its define to Date code.
Original implementations stay in place behind contiguous guards; ordinary V8
keeps them. Set this argument to `false` for a full-Date control. When disabling
the generated-body profile for comparison, disable this argument as well.

The explicit `MOONRUN_OMITTED_DATE_CPP_BUILTINS` list is shared by the existing
body profile and the new C++ stubs. It contains calendar setters, legacy getYear,
formatting methods, and Date.now/parse/UTC. Each entry keeps its original builtin
metadata and external-reference slot, now backed by a separate fatal stub.

DateConstructor, DatePrototypeToString, DatePrototypeToTemporalInstant, generated
valueOf and Symbol.toPrimitive remain. Construction and coercion still require
Date parsing, clocks, DateCache and JSDate layouts; those facilities are retained.
The change does not affect Wasm wrappers, parser removal, or compiler machinery.

## Validation and size

The matched ARM64 macOS artifacts give these incremental results relative to the
149-entry body-cut baseline:

| Component | Before | After | Saved |
| --- | ---: | ---: | ---: |
| Static archive | 60,484,120 B | 60,460,408 B | 23,712 B |
| Stripped test consumer | 15,359,312 B | 15,342,768 B | 16,544 B |
| Embedded code | 1,469,888 B | 1,469,888 B | 0 B |
| Snapshot payload | 402,096 B | 402,096 B | 0 B |

These are test-consumer measurements without V8 ThinLTO, not a full moonrun
binary or a runtime performance claim. The embedded source and snapshot are
byte-identical. All 149 selected identities, argument metadata and generated
instruction sizes are unchanged.

The original Date object contributes 20,815 bytes of text. The retained object
contributes 2,506 bytes and the unsupported stubs 517 bytes. A full-Date control
produces a Date object byte-identical to the original pre-split object.

Validation covers all 149 named traps and registrations, retained protocols in
four Wasm compilation modes, C API/Rust adapter suites and GN header dependencies.
Additional retained checks exercise numeric and ISO-string Date construction,
native value extraction, numeric/string coercion, and calling Date as a function.
Native candidate CI must validate the final PR head before landing.
