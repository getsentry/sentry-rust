<p align="center">
  <a href="https://sentry.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://sentry-brand.storage.googleapis.com/sentry-wordmark-dark-280x84.png" alt="Sentry" width="280" height="84">
  </a>
</p>

# Sentry Rust SDK: sentry-minidump

Captures native crashes as minidumps in a separate process and sends
them to Sentry as attachments.

Add [`MinidumpIntegration`] to your [`ClientOptions`]. The integration
does all of its work inside `sentry::init`:

- In the app process it spawns a crash reporter process and keeps the
  handle for the life of the client.
- In the crash reporter process it never returns. It builds its own
  client from the same options, runs the minidump server, and exits.

```rust
let _guard = sentry::init(
    sentry::ClientOptions::new()
        .dsn("https://your-dsn@sentry.io/0")
        .add_integration(
            sentry_minidump::MinidumpIntegration::new()
                .crashes_dir("/var/lib/my-app/crashes"),
        ),
);
// Only the app process reaches here.
```

Code before `sentry::init` runs in both processes, because the crash
reporter re-executes the current binary. Build the integration and call
[`MinidumpIntegration::is_crash_reporter_process`] on it to skip work
that should run only in the app process.

Initialise the minidump integration once per process. It runs a single
crash reporter for the whole process; there is no per-client isolation.
Only the first initialization that has a DSN starts the reporter; later
calls do nothing.

## Scope

The crash event carries the scope of the thread that crashed: user,
tags, extra, contexts, breadcrumbs, level, transaction and fingerprint,
after the scope's event processors have run. Nothing is sent to the
crash reporter until the crash. At that moment the crash handler names
the crashing OS thread, a helper thread serializes the scope of the hub
current on that thread, and the handler sends it to the reporter before
it requests the minidump. Hubs bound with `Hub::run` are tracked, so a
server with one hub per request reports the scope of the request that
crashed. A thread that never used Sentry falls back to the main hub.

The helper waits at most `scope_timeout` for the scope. If the crash
happened while the crashing thread held the allocator or the hub lock,
the helper cannot finish and the event goes out without the scope.
Attachments on the scope are not carried.

## Platforms

`sentry-minidump` builds on Linux, macOS and Windows only.

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
