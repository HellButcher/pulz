# pulsar-arena

A _generational arena_ allocator inspired by [generational-arena] with
compact generational indices.

When you insert a value into the arena, you get an index-pointer in
return. You can then use this index-pointer to access the provided value.

[generational-arena]: https://github.com/fitzgen/generational-arena

## Example

**TODO**

## License

This repository is licensed under either of

* MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
* Apache License, Version 2.0, ([LICENSE-APACHE](../../LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

