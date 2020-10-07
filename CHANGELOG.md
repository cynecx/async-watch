# Version 0.3.1

## Changed

- Remove `Clone` bound on channel construction (`async_watch::channel`).
- Bump min. version of `event-listener` to v2.5.1, which fixes UB (catched by miri).

# Version 0.3.0

- Renamed crate to async-watch.

# Version 0.2.0

Updated code & api according to the recent tokio's watch revisions (https://github.com/tokio-rs/tokio/pull/2814).

## New

- `Receiver::changed`.

## Changed

- Semantics of `Receiver::recv` have changed.
- Renamed `Sender::broadcast` to `Sender::send`.

# Version 0.1.0

Initial release.

