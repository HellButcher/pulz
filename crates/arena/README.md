# pulsar-arena

A _generational arena_ allocator inspired by [generational-arena] with
compact generational indices.

When you insert a value into the arena, you get an index-pointer in
return. You can then use this index-pointer to access the provided value.

[generational-arena]: https://github.com/fitzgen/generational-arena

## Example

```rust
use pulsar_arena::{Arena,Index};

let mut arena = Arena::new();

// insert some elements and remember the returned index
let a = arena.insert("foo");
let b = arena.insert("bar");

// access inserted elements by returned index
assert_eq!("bar", arena[b]);

// there are also the "checked" versions `get` and `get_mut` that returns Option. 
assert_eq!(Some(&"foo"), arena.get(a));

// items can be removed efficiently
assert_eq!(Some("foo"), arena.remove(a));
assert!(!arena.contains(a));
```

## `no_std`

This crate should also work without `std`. No additional configuration required.

## License

This repository is licensed under either of

* MIT license ([LICENSE-MIT] or <http://opensource.org/licenses/MIT>)
* Apache License, Version 2.0, ([LICENSE-APACHE] or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[LICENSE-MIT]: ../../LICENSE-MIT
[LICENSE-APACHE]: ../../LICENSE-APACHE
