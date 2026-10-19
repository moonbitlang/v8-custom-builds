# Explicit omitted builtin bodies

This is a measurement record for the pinned experiment, not a name-prefix rule.
The patch header is the authoritative selection. Sizes below are generated
instruction bytes (excluding alignment), from each Code object. All listed
builtins remain distinct and keep their original ID and argument metadata.

## Array

See the Array dependency rationale in [README.md](README.md#selection-and-dependency-rationale). Shared helpers and continuations remain.

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

See the TypedArray dependency rationale in [README.md](README.md#selection-and-dependency-rationale). Shared helpers and continuations remain.

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

See the Set dependency rationale in [README.md](README.md#selection-and-dependency-rationale). Shared helpers and continuations remain.

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

See the Iterator dependency rationale in [README.md](README.md#selection-and-dependency-rationale). Shared helpers and continuations remain.

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

See the Promise dependency rationale in [README.md](README.md#selection-and-dependency-rationale). Shared helpers and continuations remain.

| Builtin | Method | Before | After |
| --- | --- | ---: | ---: |
| `PromiseAll` | `all` | 4,132 | 24 |
| `PromiseAllSettled` | `allSettled` | 4,464 | 24 |
| `PromiseAny` | `any` | 2,696 | 24 |
| `PromiseRace` | `race` | 1,536 | 24 |
| `PromiseWithResolvers` | `withResolvers` | 332 | 24 |
