# async-watch2

[![Build](https://github.com/cynecx/async-watch/workflows/CI/badge.svg)](
https://github.com/cynecx/async-watch/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/cynecx/async-watch)
[![Cargo](https://img.shields.io/crates/v/async-watch2.svg)](
https://crates.io/crates/async-watch2)
[![Documentation](https://docs.rs/async-watch2/badge.svg)](
https://docs.rs/async-watch2)

A single-producer, multi-consumer channel that only retains the *last* sent value.

Extracted from [Tokio's](https://github.com/tokio-rs/tokio/) [`tokio::sync::watch`](https://github.com/tokio-rs/tokio/blob/master/tokio/src/sync/watch.rs) implementation,
which was initially written by [Carl Lerche](https://github.com/carllerche).

## License

async-watch2 is primarily distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
