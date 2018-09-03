# Changelog

# 0.9.0

* Removed `drain_events`.  Events are now drained by calling `Client::close` or on the
  transport on `Transport::shutdown`.
* Removed `Hub::add_event_processor`.  This was replaced by `Scope::add_event_processor`
  which is easier to use (only returns factory function)/
* Added various new client configuration values.
* Unified option handling

This is likely to be the final API before 1.0

# 0.3.1

* Remove null byte terminator from device model context (#33)
* Fix `uname` breaking builds on Windows (#32)
* Fix the crate documentation link (#31)
