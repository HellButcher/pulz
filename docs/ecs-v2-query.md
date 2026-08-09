# ECS v2 Query System

## Trait hierarchy

```
QueryFetchState         — cached per-query-type metadata (ResourceIds, ComponentIds)
QueryFilter             — archetype matching + associated Fetch type
  └── QueryData         — item type declaration (what you get per entity)
        └── (impl via) QueryFetch — runtime borrow + per-entity access
```

The key insight: **`QueryFilter` and `QueryData` are purely type-level**. They carry no borrowed data. All runtime work lives in `QueryFetch`, which holds the actual borrowed storage handles.

---

## Trait responsibilities

### `QueryFetchState`

```rust
pub trait QueryFetchState: Send + Sync + 'static {
    fn init(res: &Resources, components: &Components) -> Self;
    fn update_access(&self, access: &mut ResourceAccess);
}
```

- Initialized **once per query type** when the query is first registered as a resource
- Stores `ResourceId<T::Storage>` and `ComponentId<T>` looked up at init time
- Reused across every invocation of the same query type — no per-frame allocation
- `update_access` declares which resources the query reads/writes (for system parallelism)

### `QueryFilter`

```rust
pub trait QueryFilter {
    type State: QueryFetchState;
    type Fetch<'w>: QueryFetch<State = Self::State>;
    fn matches_archetype(state: &Self::State, archetype: &Archetype) -> bool;
}
```

- Decides which archetypes a query iterates over
- **Pure filters** (`With<T>`, `Without<T>`) implement this without a data fetch — zero cost
- The separation from `QueryData` means filters can be composed freely without coupling to item types
- `matches_archetype` is called during `QueryState::update_archetypes` to build the cached `ArchetypeSet`

### `QueryData: QueryFilter`

```rust
pub trait QueryData: QueryFilter {
    type Item<'a>;
}
```

- Declares the type of value yielded per entity (`&'a T`, `(&'a A, &'a mut B)`, etc.)
- No methods — purely a type-level declaration
- Implemented by user-facing query types: `&T`, `&mut T`, `(A, B, ...)`, `Option<Q>`, `Entity`

### `QueryFetch`

```rust
pub trait QueryFetch: Sized + Send {
    type State: QueryFetchState;
    type Item<'a> where Self: 'a;

    fn fetch(res: &ResourcesSend, state: &Self::State) -> Self;
    fn set_archetype(&mut self, state: &Self::State, archetype: &Archetype);
    fn get<'a>(&'a mut self, archetype: &Archetype, index: usize) -> Self::Item<'a>;
}
```

- Holds actual borrowed storage handles (`Res<T::Storage>`, `ResMut<T::Storage>`)
- `fetch` acquires borrows from `Resources` — called once per `Query` construction
- `set_archetype` is called once per archetype during iteration (caches archetype-local state)
- `get` extracts one item at a given slot index — called once per entity

---

## Implementations at a glance

| User type | `matches_archetype` | `QueryFetch::fetch` | `QueryFetch::get` |
|---|---|---|---|
| `&T` | archetype has T | borrow `Res<T::Storage>` | `storage.get(entity, arch, idx)` |
| `&mut T` | archetype has T | borrow `ResMut<T::Storage>` | `storage.get_mut(...)` |
| `Entity` | always true | nothing | `archetype.entities[idx]` |
| `Option<Q>` | always true | same as Q | `Some(q.get(...))` or `None` |
| `With<T, Q>` | T present AND Q matches | same as Q | same as Q |
| `Without<T, Q>` | T absent AND Q matches | same as Q | same as Q |
| `(A, B, ...)` | all match | all fetch | all get, return tuple |

---

## QueryState and Query execution

`QueryState<Q>` is a resource stored in `Resources`. It caches:
- The `Q::State` (init'd once)
- An `ArchetypeSet` of matching archetypes (lazily updated as new archetypes are created)
- The last-seen archetype count (to detect new archetypes without re-checking old ones)

`Query<'w, Q>` is the user-facing executor. It holds:
- `Res<'w, WorldInner>` — to read entity locations and archetype data
- `Res<'w, QueryState<Q>>` — the cached matching archetypes
- `Q::Fetch<'w>` — the borrowed storage handles

Iteration walks the matching `ArchetypeSet`, calls `set_archetype` once per archetype, then `get` for each entity slot within it.

---

## Why separate QueryFilter from QueryData?

In ecs-v1, `QueryParamState::matches_archetype` was fused with the state that also described fetch behavior. This meant every filter had to carry storage borrow machinery even if it fetched nothing.

In ecs-v2, `QueryFilter` can be implemented by zero-sized types (`With<T>`, `Without<T>`) with no storage access at all. The `QueryData` layer adds item extraction on top, only where needed. This makes pure-filter queries cheaper and the trait hierarchy easier to reason about.
