# Explicit unsupported builtin bodies

This is an explicit selection, not a prefix rule. The patch header is authoritative.
Sizes are instruction bytes, excluding alignment. All entries retain their IDs
and argument metadata. For CPP entries the report uses -1 for the TFJ-only stack
parameter field; Code metadata and the C++ dispatch entry are checked separately.

## Array

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `ArrayEvery` | `every` | 1,572 | 24 |
| `ArrayFilter` | `filter` | 4,416 | 24 |
| `ArrayForEach` | `forEach` | 1,376 | 24 |
| `ArrayMap` | `map` | 3,876 | 24 |
| `ArrayReduce` | `reduce` | 1,576 | 24 |
| `ArrayReduceRight` | `reduceRight` | 1,860 | 24 |
| `ArraySome` | `some` | 1,548 | 24 |
| `ArrayPrototypeAt` | `at` | 852 | 24 |
| `ArrayPrototypeCopyWithin` | `copyWithin` | 4,504 | 24 |
| `ArrayPrototypeFind` | `find` | 1,536 | 24 |
| `ArrayPrototypeFindIndex` | `findIndex` | 1,536 | 24 |
| `ArrayPrototypeFindLast` | `findLast` | 1,656 | 24 |
| `ArrayPrototypeFindLastIndex` | `findLastIndex` | 1,652 | 24 |
| `ArrayPrototypeFlat` | `flat` | 852 | 24 |
| `ArrayPrototypeFlatMap` | `flatMap` | 820 | 24 |
| `ArrayPrototypeLastIndexOf` | `lastIndexOf` | 3,948 | 24 |
| `ArrayPrototypeReverse` | `reverse` | 3,208 | 24 |
| `ArrayPrototypeShift` | `shift` | 3,440 | 24 |
| `ArrayPrototypeSlice` | `slice` | 6,048 | 24 |
| `ArrayPrototypeSort` | `sort` | 1,784 | 24 |
| `ArrayPrototypeSplice` | `splice` | 12,608 | 24 |
| `ArrayPrototypeToReversed` | `toReversed` | 2,848 | 24 |
| `ArrayPrototypeToSorted` | `toSorted` | 1,880 | 24 |
| `ArrayPrototypeToSpliced` | `toSpliced` | 8,312 | 24 |
| `ArrayPrototypeUnshift` | `unshift` | 2,808 | 24 |
| `ArrayPrototypeWith` | `with` | 1,128 | 24 |

## TypedArray

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `TypedArrayPrototypeAt` | `at` | 1,060 | 24 |
| `TypedArrayPrototypeEvery` | `every` | 1,736 | 24 |
| `TypedArrayPrototypeFilter` | `filter` | 4,316 | 24 |
| `TypedArrayPrototypeFind` | `find` | 1,708 | 24 |
| `TypedArrayPrototypeFindIndex` | `findIndex` | 1,712 | 24 |
| `TypedArrayPrototypeFindLast` | `findLast` | 1,684 | 24 |
| `TypedArrayPrototypeFindLastIndex` | `findLastIndex` | 1,688 | 24 |
| `TypedArrayPrototypeForEach` | `forEach` | 1,576 | 24 |
| `TypedArrayPrototypeMap` | `map` | 28,688 | 24 |
| `TypedArrayPrototypeReduce` | `reduce` | 1,684 | 24 |
| `TypedArrayPrototypeReduceRight` | `reduceRight` | 1,672 | 24 |
| `TypedArrayPrototypeSlice` | `slice` | 3,800 | 24 |
| `TypedArrayPrototypeSome` | `some` | 1,712 | 24 |
| `TypedArrayPrototypeSort` | `sort` | 2,192 | 24 |
| `TypedArrayPrototypeToReversed` | `toReversed` | 1,672 | 24 |
| `TypedArrayPrototypeToSorted` | `toSorted` | 3,128 | 24 |
| `TypedArrayPrototypeWith` | `with` | 2,900 | 24 |

## Set

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `SetPrototypeDifference` | `difference` | 11,176 | 24 |
| `SetPrototypeIntersection` | `intersection` | 12,104 | 24 |
| `SetPrototypeSymmetricDifference` | `symmetricDifference` | 11,192 | 24 |
| `SetPrototypeUnion` | `union` | 7,208 | 24 |
| `SetPrototypeIsDisjointFrom` | `isDisjointFrom` | 2,924 | 24 |
| `SetPrototypeIsSubsetOf` | `isSubsetOf` | 1,724 | 24 |
| `SetPrototypeIsSupersetOf` | `isSupersetOf` | 2,220 | 24 |

## Iterator

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `IteratorPrototypeMap` | `map` | 388 | 24 |
| `IteratorPrototypeFilter` | `filter` | 388 | 24 |
| `IteratorPrototypeTake` | `take` | 500 | 24 |
| `IteratorPrototypeDrop` | `drop` | 500 | 24 |
| `IteratorPrototypeFlatMap` | `flatMap` | 404 | 24 |
| `IteratorPrototypeReduce` | `reduce` | 1,740 | 24 |
| `IteratorPrototypeToArray` | `toArray` | 1,564 | 24 |
| `IteratorPrototypeForEach` | `forEach` | 1,080 | 24 |
| `IteratorPrototypeSome` | `some` | 1,364 | 24 |
| `IteratorPrototypeEvery` | `every` | 1,360 | 24 |
| `IteratorPrototypeFind` | `find` | 1,372 | 24 |

## Promise

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `PromiseAll` | `all` | 4,132 | 24 |
| `PromiseAllSettled` | `allSettled` | 4,464 | 24 |
| `PromiseAny` | `any` | 2,696 | 24 |
| `PromiseRace` | `race` | 1,536 | 24 |
| `PromiseWithResolvers` | `withResolvers` | 332 | 24 |

## ArrayConstructor

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `ArrayFrom` | `from` | 4,760 | 24 |
| `ArrayFromAsync` | `fromAsync` | 15,784 | 24 |
| `ArrayOf` | `of` | 1,748 | 24 |

## MapConstructor

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `MapGroupBy` | `groupBy` | 7,716 | 24 |

## ObjectConstructor

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `ObjectAssign` | `assign` | 4,108 | 24 |
| `ObjectEntries` | `entries` | 1,468 | 24 |
| `ObjectValues` | `values` | 1,224 | 24 |
| `ObjectKeys` | `keys` | 852 | 24 |
| `ObjectFromEntries` | `fromEntries` | 3,400 | 24 |
| `ObjectGroupBy` | `groupBy` | 7,984 | 24 |

## Reflect

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `ReflectIsExtensible` | `isExtensible` | 228 | 24 |
| `ReflectPreventExtensions` | `preventExtensions` | 232 | 24 |
| `ReflectGetPrototypeOf` | `getPrototypeOf` | 208 | 24 |
| `ReflectSetPrototypeOf` | `setPrototypeOf` | 336 | 24 |
| `ReflectGet` | `get` | 312 | 24 |
| `ReflectDeleteProperty` | `deleteProperty` | 176 | 24 |
| `ReflectHas` | `has` | 172 | 24 |
| `ReflectGetOwnPropertyDescriptor` | `getOwnPropertyDescriptor` | 4,392 | 24 |
| `ReflectDefineProperty` | `defineProperty` | 8 | 4 |
| `ReflectOwnKeys` | `ownKeys` | 8 | 4 |
| `ReflectSet` | `set` | 8 | 4 |

## Date

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `DatePrototypeGetDate` | `getDate` | 196 | 24 |
| `DatePrototypeGetDay` | `getDay` | 196 | 24 |
| `DatePrototypeGetFullYear` | `getFullYear` | 196 | 24 |
| `DatePrototypeGetHours` | `getHours` | 196 | 24 |
| `DatePrototypeGetMilliseconds` | `getMilliseconds` | 168 | 24 |
| `DatePrototypeGetMinutes` | `getMinutes` | 196 | 24 |
| `DatePrototypeGetMonth` | `getMonth` | 196 | 24 |
| `DatePrototypeGetSeconds` | `getSeconds` | 196 | 24 |
| `DatePrototypeGetTime` | `getTime` | 252 | 24 |
| `DatePrototypeGetTimezoneOffset` | `getTimezoneOffset` | 168 | 24 |
| `DatePrototypeGetUTCDate` | `getUTCDate` | 168 | 24 |
| `DatePrototypeGetUTCDay` | `getUTCDay` | 168 | 24 |
| `DatePrototypeGetUTCFullYear` | `getUTCFullYear` | 168 | 24 |
| `DatePrototypeGetUTCHours` | `getUTCHours` | 168 | 24 |
| `DatePrototypeGetUTCMilliseconds` | `getUTCMilliseconds` | 168 | 24 |
| `DatePrototypeGetUTCMinutes` | `getUTCMinutes` | 168 | 24 |
| `DatePrototypeGetUTCMonth` | `getUTCMonth` | 168 | 24 |
| `DatePrototypeGetUTCSeconds` | `getUTCSeconds` | 168 | 24 |
| `DatePrototypeGetYear` | `getYear` | 8 | 4 |
| `DatePrototypeSetYear` | `setYear` | 8 | 4 |
| `DatePrototypeSetDate` | `setDate` | 8 | 4 |
| `DatePrototypeSetFullYear` | `setFullYear` | 8 | 4 |
| `DatePrototypeSetHours` | `setHours` | 8 | 4 |
| `DatePrototypeSetMilliseconds` | `setMilliseconds` | 8 | 4 |
| `DatePrototypeSetMinutes` | `setMinutes` | 8 | 4 |
| `DatePrototypeSetMonth` | `setMonth` | 8 | 4 |
| `DatePrototypeSetSeconds` | `setSeconds` | 8 | 4 |
| `DatePrototypeSetTime` | `setTime` | 8 | 4 |
| `DatePrototypeSetUTCDate` | `setUTCDate` | 8 | 4 |
| `DatePrototypeSetUTCFullYear` | `setUTCFullYear` | 8 | 4 |
| `DatePrototypeSetUTCHours` | `setUTCHours` | 8 | 4 |
| `DatePrototypeSetUTCMilliseconds` | `setUTCMilliseconds` | 8 | 4 |
| `DatePrototypeSetUTCMinutes` | `setUTCMinutes` | 8 | 4 |
| `DatePrototypeSetUTCMonth` | `setUTCMonth` | 8 | 4 |
| `DatePrototypeSetUTCSeconds` | `setUTCSeconds` | 8 | 4 |
| `DatePrototypeToDateString` | `toDateString` | 8 | 4 |
| `DatePrototypeToISOString` | `toISOString` | 8 | 4 |
| `DatePrototypeToUTCString` | `toUTCString` | 8 | 4 |
| `DatePrototypeToTimeString` | `toTimeString` | 8 | 4 |
| `DatePrototypeToJson` | `toJSON` | 8 | 4 |

## DateConstructor

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `DateNow` | `now` | 8 | 4 |
| `DateParse` | `parse` | 8 | 4 |
| `DateUTC` | `UTC` | 8 | 4 |

## Number

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `NumberPrototypeToExponential` | `toExponential` | 8 | 4 |
| `NumberPrototypeToFixed` | `toFixed` | 8 | 4 |
| `NumberPrototypeToPrecision` | `toPrecision` | 8 | 4 |
| `NumberPrototypeToLocaleString` | `toLocaleString` | 8 | 4 |

## JSON

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `JsonParse` | `parse` | 8 | 4 |
| `JsonStringify` | `stringify` | 8 | 4 |

## String

See the dependency rationale in [README.md](README.md#selection-and-dependency-rationale).

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `StringPrototypeAnchor` | `anchor` | 180 | 24 |
| `StringPrototypeBig` | `big` | 148 | 24 |
| `StringPrototypeBlink` | `blink` | 148 | 24 |
| `StringPrototypeBold` | `bold` | 148 | 24 |
| `StringPrototypeFontcolor` | `fontcolor` | 180 | 24 |
| `StringPrototypeFontsize` | `fontsize` | 180 | 24 |
| `StringPrototypeFixed` | `fixed` | 148 | 24 |
| `StringPrototypeItalics` | `italics` | 148 | 24 |
| `StringPrototypeLink` | `link` | 180 | 24 |
| `StringPrototypeSmall` | `small` | 148 | 24 |
| `StringPrototypeStrike` | `strike` | 148 | 24 |
| `StringPrototypeSub` | `sub` | 148 | 24 |
| `StringPrototypeSup` | `sup` | 148 | 24 |
