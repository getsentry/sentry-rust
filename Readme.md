Rust Sentry Lib
==========

[![Build Status](https://travis-ci.org/getsentry/sentry-rust.svg?branch=master)](https://travis-ci.org/getsentry/sentry-rust)
[![Crates.io](https://img.shields.io/crates/v/sentry.svg?style=flat)](https://crates.io/crates/sentry)
[![Coverage Status](https://coveralls.io/repos/github/getsentry/sentry-rust/badge.svg?branch=master)](https://coveralls.io/github/getsentry/sentry-rust?branch=master)


[Sentry Service](https://www.getsentry.com/) now available for rust ;)

Rust 1.10 should include register_panic_handler and btw bring more value to this lib ;)
This implementation use one thread listening incoming messages from dedicated channel and sending those messages to sentry server.
If this thread panics, a new one is created.


## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
sentry = "0.2.0"
```

and this to your crate root:

```rust
extern crate sentry;
```

## Examples

```rust
let creds = "https://mypublickey:myprivatekey@myhost/myprojectid"
    .parse::<SentryCredential>()
    .unwrap();
let sentry = Sentry::from_settings(handle, Default::default(), creds);
sentry.info("test.logger", "Test Message", None);
```

alternatively, you can specify optional settings such as device information

```rust
let creds = "https://mypublickey:myprivatekey@myhost/myprojectid"
    .parse::<SentryCredential>()
    .unwrap();
let device = Device::new("device_name".to_string(), "version".to_string(), "build".to_string());
let settings = Settings {
    device: device,
    ..Settings::default()
};
let sentry = Sentry::from_settings(handle, Default::default(), creds);
sentry.info("test.logger", "Test Message", None);
```

you can share sentry accross threads

```rust
let creds = "https://mypublickey:myprivatekey@myhost/myprojectid"
    .parse::<SentryCredential>()
    .unwrap();
let sentry = Sentry::from_settings(handle, Default::default(), creds);

let sentry1 = sentry.clone();
thread::spawn(move || sentry1.info("test.logger", "Test Message", None));
```

with rust 1.10 or nightly you can register panic handler and still provide you own handler

```rust
sentry.register_panic_handler(Some(|panic_info: &PanicInfo| -> () {}));
sentry.unregister_panic_handler();
```

## OpenSSL

Check [OpenSSL setup](https://github.com/sfackler/rust-openssl/blob/b8fb29db5c246175a096260eacca38180cd77dd0/README.md)
for OSX if you have some issue while building OpenSSL.

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
