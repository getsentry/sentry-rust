.. sentry:edition:: self

    Sentry Rust
    ===========

.. sentry:edition:: hosted, on-premise

    .. class:: platform-rust

    Rust
    ====

Sentry-Rust is the official Rust SDK for Sentry.  It maps the entire
Sentry protocol for Rust and provides convenient helpers for sending
common types of events to Sentry.

Installation
------------

Sentry-Rust is distributed as a normal crate from crates.io.  You can add
it to your project as a dependency in your `Cargo.toml` file::

    [dependencies]
    sentry = "###SENTRY_VERSION###"

Additionally you can configure a bunch of features to enable or disable
functionality in the crate.  By default the most common features are
compiled into the crate.  For a list of features that are available refer
to the `API Documentation <https://docs.rs/sentry>`__.

Configuring the Client
----------------------

The client is configured by calling ``sentry::init`` with a value that can
be converted into a configuration object.  These are the most common
values:

*   *an empty tuple*: in that case the client is configured from the
    :envvar:`SENTRY_DSN` environment variable.
*   *a string holding a DSN*: if you pass a string then a DSN is parsed
    and the client is initialized with that DSN.
*   *a tuple in the form (dsn, options)*: This is a form where the client
    is configured with a DSN plus an options object that allows you to
    configure additional features.

This is the most common case for client configuration:

.. sourcecode:: rust

    extern crate sentry;

    fn main() {
        sentry::init("___PUBLIC_DSN___");
        // code using sentry goes here.
    }

To configure releases automatically you can use the
``sentry_crate_release!`` macro in combination with the tuple config
syntax:

.. sourcecode:: rust

    #[macro_use] extern crate sentry;

    fn main() {
        sentry::init(("___PUBLIC_DSN___", sentry::ClientOptions {
            release: sentry_crate_release!(),
            ..Default::default()
        }));
        // code using sentry goes here.
    }

Reporting Errors
----------------

Once Sentry is configured errors and other events can be emitted.  Since
Rust has different mechanisms by which errors can be issued different
functionality is provided for them.  By default support for the new
`failure <https://docs.rs/failure>`__ error system is provided.

For instance to report a ``failure::Error`` this code can be used:

.. sourcecode:: rust

    use sentry::integrations::failure::capture_error;

    let result = match a_function_that_might_fail() {
        Ok(val) => val,
        Err(err) => {
            capture_error(&err);
            return Err(err);
        }
    };


Catching Panics
---------------

To automatically catch panics the panic integration can be used:

.. sourcecode:: rust

    use sentry::integrations::panic::register_panic_handler;

    fn main() {
        sentry::init(...);
        register_panic_handler();
    }

Flushing Errors on Shutdown
---------------------------

Since Sentry Rust uses a thread to offload event sending it's possible
that pending events are not sent on shutdown.  There are two ways to
prevent this from happening.  The first one is to retain the guard
returned from ``sentry::init`` which will flush out in the `Drop`
implementation (it will wait up to 2 seconds for this):

.. sourcecode:: rust

    fn main() {
        let _guard = sentry::init(...);
    }

More Information
----------------

For more information (in particular about how to set contextual data)
refer to the `full API documentation <https://docs.rs/sentry/>`__ which
has all that information and more.

Resources:

* `API Docs <http://docs.rs/sentry>`_
* `Crates.io page <http://crates.io/crates/sentry>`_
* `Bug Tracker <http://github.com/getsentry/sentry-rust/issues>`_
* `Github Project <http://github.com/getsentry/sentry-rust>`_
