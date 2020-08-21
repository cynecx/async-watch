# async-watch2

A single-producer, multi-consumer channel that only retains the *last* sent value.

Extracted from [Tokio's](https://github.com/tokio-rs/tokio/) [`tokio::sync::watch`](https://github.com/tokio-rs/tokio/blob/master/tokio/src/sync/watch.rs) implementation,
which was initially written by [Carl Lerche](https://github.com/carllerche).

## License

async-watch2 is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
