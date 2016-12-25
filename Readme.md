Rust Sentry Lib
==========

[![Build Status](https://travis-ci.org/aagahi/rust-sentry.svg?branch=master)](https://travis-ci.org/aagahi/rust-sentry)
[![Crates.io](https://img.shields.io/crates/v/sentry.svg?style=flat)](https://crates.io/crates/sentry)
[![Coverage Status](https://coveralls.io/repos/github/aagahi/rust-sentry/badge.svg?branch=master)](https://coveralls.io/github/aagahi/rust-sentry?branch=master)


[Sentry Service](https://www.getsentry.com/) now available for rust ;)

Rust 1.10 should include register_panic_handler and btw bring more value to this lib ;)
This implementation use one thread listening incoming messages from dedicated channel and sending those messages to sentry server.
If this thread panics, a new one is created.


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
sentry = "0.1.9"
```

and this to your crate root:

```rust
extern crate sentry;
```

## Examples

```rust
let credential = SentryCredential { key: "xx".to_string(), secret: "xx".to_string(), host: "app.getsentry.com".to_string(), project_id: "xx".to_string() };
let sentry = Sentry::new( "Server Name".to_string(), "release".to_string(), "test_env".to_string(), credential );
sentry.info("test.logger", "Test Message", None);
```

you can share sentry accross threads

```rust
let sentry = Arc::new(Sentry::new( "Server Name".to_string(), "release".to_string(), "test_env".to_string(), credential ));
let sentry1 = sentry.clone();
thread::spawn(move || sentry1.info("test.logger", "Test Message", None));
```

with rust 1.10 or nightly you can register panic handler and still provide you own handler

```rust
sentry.register_panic_handler(Some(|panic_info: &PanicInfo| -> () {}));
sentry.unregister_panic_handler();
```


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
