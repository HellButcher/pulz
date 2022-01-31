# `pulz`

<img align="right" src="https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo-full.png"/>

[![license: MIT/Apache 2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![Rust CI](https://github.com/HellButcher/pulz/actions/workflows/rust.yml/badge.svg)](https://github.com/HellButcher/pulz/actions/workflows/rust.yml)


A collection of rust crates for game-development.

## Crates

* **[`pulz-arena`](crates/arena)** -
  A _generational arena_ allocator with compact generational indices
  <mark>DISCONTINUED</mark>

  [![Crates.io](https://img.shields.io/crates/v/pulz-arena.svg?label=pulz-arena)](https://crates.io/crates/pulz-arena)
  [![docs.rs](https://docs.rs/pulz-arena/badge.svg)](https://docs.rs/pulz-arena/)

* **[`pulz-executor`](crates/executor)** -
  Abstractions of some async runtimes

  [![Crates.io](https://img.shields.io/crates/v/pulz-executor.svg?label=pulz-executor)](https://crates.io/crates/pulz-executor)
  [![docs.rs](https://docs.rs/pulz-executor/badge.svg)](https://docs.rs/pulz-executor/)

* **[`pulz-schedule`](crates/schedule)** -
  For scheduling systems and managing their resources

  [![Crates.io](https://img.shields.io/crates/v/pulz-schedule.svg?label=pulz-schedule)](https://crates.io/crates/pulz-schedule)
  [![docs.rs](https://docs.rs/pulz-schedule/badge.svg)](https://docs.rs/pulz-schedule/)

* **[`pulz-ecs`](crates/ecs)** -
  An _archetype_ based ECS (Entity Component System)

  [![Crates.io](https://img.shields.io/crates/v/pulz-ecs.svg?label=pulz-ecs)](https://crates.io/crates/pulz-ecs)
  [![docs.rs](https://docs.rs/pulz-ecs/badge.svg)](https://docs.rs/pulz-ecs/)

## License

[license]: #license

This repository is licensed under either of

* MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)
* Apache License, Version 2.0, ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[LICENSE-MIT]: LICENSE-MIT
[LICENSE-APACHE]: LICENSE-APACHE
