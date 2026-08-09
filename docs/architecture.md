# ECS v2 Architecture

## Core idea

World state lives inside `pulz_schedule::Resources`. Every subsystem (entity list, component registry, archetypes, each component storage) is an independent resource. This enables fine-grained borrowing: multiple systems can hold different storage borrows simultaneously without global locking.

## Key types

```
Resources
 ├── WorldInner          — entity list, component registry, archetype registry
 ├── WorldMutInnerTemp   — scratch ComponentSets for pending insert/remove
 ├── ArchetypeStorage<A> — column store for component A (archetype layout)
 ├── SparseStorage<B>    — sparse secondary map for component B
 └── ...                 — one resource per component type's storage
```

### WorldInner

Holds `Entities`, `Components`, `Archetypes`. Does **not** hold storage — storages are separate resources. `WorldMut` temporarily `Taken`-owns `WorldInner` so it can be borrowed mutably alongside other resources.

### Entity lifecycle

1. `WorldMut::spawn()` → allocate entity in `Entities`, place in `EMPTY` archetype, return `EntityMut`
2. `EntityMut::insert::<T>(value)` → stage value in `T::Storage::tmp`, mark component in `WorldMutInnerTemp::tmp_inserted`
3. On `EntityMut::flush()` (lazy, called before any read or on drop):
   - Compute new `ComponentSet = old ∪ inserted − removed`
   - Find/create the target `ArchetypeId`
   - Move each archetype-storage column: `swap_remove_and_push(old_arch, old_idx, new_arch)`
   - `flush_push` newly inserted components into the new archetype column
   - Update entity location

### Archetype system

An **archetype** groups entities that share the exact same set of archetype-tracked components. Sparse-storage components do not affect archetype identity (they are stripped out before lookup).

Archetype identity is keyed by an interned `ComponentSet` (`RoaringBitmap`). `ArchetypeId` is a dense `u32` index into a `Vec<Archetype>`. `ArchetypeMap<T>` is a `Vec<T>` indexed by `ArchetypeId`, used by storages for O(1) column lookup.

### Storage types

| Type | Layout | `SPARSE` | Use case |
|---|---|---|---|
| `ArchetypeStorage<T>` | `ArchetypeMap<Vec<T>>` — one column per archetype | false | most components |
| `SparseStorage<T>` | `SparseSecondaryMap<Entity, T>` | true | rare/optional components |
| `SlotStorage<T>` | `SlotMap<Entity, T>` | true | components needing stable IDs |
| `Tracked<S>` | wraps any storage, records `removed: Vec<Entity>` | same as S | change detection |

The `Storage` trait defines a two-phase insert: `insert` stages the value, `flush_push`/`flush_replace` commit it once the final archetype is known. This avoids knowing the target archetype at insert time.

## Design decisions

**`WorldMutInnerTemp` is a separate resource** (not inside `WorldInner`) so that `flush()` can hold `&mut WorldInner` and `&mut WorldMutInnerTemp` simultaneously while other code holds borrows into `WorldInner` (e.g. archetype references). In ecs-v1 these were fused, requiring `UnsafeCell` around the entire world.

**`ComponentData::borrow_mut_any_storage_fn` is a safe `fn` pointer** (not an `unsafe` function pointer as in ecs-v1). The `Storage::borrow_mut_any` associated function absorbs the cast inside `ResMut::map`, which is already an unsafe abstraction boundary. Callers never need to write `unsafe`.

**`get_disjoint_mut` uses stable std** (`slice::get_disjoint_mut`) instead of the custom raw-pointer helper in ecs-v1.
