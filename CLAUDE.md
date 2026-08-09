# pulz-ecs — Claude Code Guide

## What this project is

A Rust ECS (Entity Component System) and game engine foundation in rust. 

Uses cargo.
This is a Cargo-workspace.
Members defined in `/crates/` directory.

## Important crates

| Crate | Path | Descr. |
|---|---|---|
| `pulz-ecs` | `crates/ecs/` | ECS impl. on top of pulz-schedule |
| `pulz-schedule` | `crates/schedule/` | Resource management and parrallel systems scheduling |
| `pulz-app` | `crates/app/` | application framework on top |

## Build & test

```sh
cargo build
cargo test
cargo clippy -- -D warnings
```

## Key conventions

- All `unsafe` blocks must have a `// SAFETY:` comment explaining the invariant
- `ComponentId<T>` is a phantom-typed `u32` index — use `.untyped()` / `.typed()` to cast
- `WorldMut` takes `WorldInner` out of `Resources` via `Taken<T>` — don't hold both alive
- Storage resources live in `pulz_schedule::Resources`; borrow them via `res.borrow_res_id(storage_id)`
- New components must be registered via `world.init::<T>()` before use

## Architecture docs

- [`docs/architecture.md`](docs/architecture.md) — overall ECS design
- [`docs/ecs-v2-query.md`](docs/ecs-v2-query.md) — query system design and trait responsibilities
