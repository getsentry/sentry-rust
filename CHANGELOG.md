# Changelog

## 0.12.0

- Upgrade reqwest to 0.9
- Add support for debug logging through the log crate (`with_debug_to_log` feature)
- Added debug log for when events are dropped in the transport.

## 0.11.1

- Fix compilation error in `sentry-actix` (#87)

## 0.11.0

- Added `sentry::with_scope`
- Updated the sentry types to 0.8.x

## 0.10.0

- Streamline types with other SDKs. Most notabe changes:
  - `Event::id` has been renamed to `Event::event_id`
  - `Event::exceptions` has been renamed to `Event::exception`
  - Some collections are now wrapped in `Values`
- Added more debug logs.

## 0.9.0

- Removed `drain_events`. Events are now drained by calling `Client::close` or on the
  transport on `Transport::shutdown`.
- Removed `Hub::add_event_processor`. This was replaced by `Scope::add_event_processor`
  which is easier to use (only returns factory function)/
- Added various new client configuration values.
- Unified option handling

This is likely to be the final API before 1.0

## 0.3.1

- Remove null byte terminator from device model context (#33)
- Fix `uname` breaking builds on Windows (#32)
- Fix the crate documentation link (#31)
