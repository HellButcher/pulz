# `pulz-bitset` 

<img align="right" src="https://raw.githubusercontent.com/HellButcher/pulz/master/docs/logo-full.png"/>

[![Crates.io](https://img.shields.io/crates/v/pulz-bitset.svg?label=pulz-bitset)](https://crates.io/crates/pulz-bitset)
[![docs.rs](https://docs.rs/pulz-bitset/badge.svg)](https://docs.rs/pulz-bitset/)
[![license: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![Rust CI](https://github.com/HellButcher/pulz/actions/workflows/rust.yml/badge.svg)](https://github.com/HellButcher/pulz/actions/workflows/rust.yml)

A simple bitset implementation.

## Example

```rust
use pulz_bitset::BitSet;

let mut bitset = BitSet::new();

assert!(!bitset.contains(1));
assert!(!bitset.contains(1337));

// insert new value
assert!(bitset.insert(1337));
assert!(!bitset.contains(1));
assert!(bitset.contains(1337));

// insert an other value
assert!(bitset.insert(1));
assert!(bitset.contains(1));

// inserting an already existing value returns false
assert!(!bitset.insert(1));
// removing a value, that was not inserted
assert!(!bitset.remove(333));
// removing an inserted value
assert!(bitset.remove(1337));
// removing a value, that was already removed
assert!(!bitset.remove(1337));

assert!(bitset.contains(1));
assert!(!bitset.contains(333));
assert!(!bitset.contains(1337));
```

## License

[license]: #license

This project is licensed under either of

* MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)
* Apache License, Version 2.0, ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[LICENSE-MIT]: ../../LICENSE-MIT
[LICENSE-APACHE]: ../../LICENSE-APACHE
