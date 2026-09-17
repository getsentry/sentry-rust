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
If the same instance is passed to `sentry::init` more than once, only the
first call that has a DSN starts the reporter; later calls do nothing.

## Scope sync

Scope changes do not cross the process boundary on their own. Send them
to the crash reporter through the integration:

```rust
sentry::with_integration(|minidump: &sentry_minidump::MinidumpIntegration, _| {
    minidump.set_user(Some(user.clone()));
});
```

## Platforms

`sentry-minidump` builds on Linux, macOS and Windows only.

## Resources

License: MIT

- [Discord](https://discord.gg/ez5KZN7) server for project discussions.
- Follow [@sentry](https://x.com/sentry) on X for updates.
