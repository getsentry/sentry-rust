# PR 957 review findings (updated scope: not targeting #946)

## PR context
- PR: #957 “fix!(core): Make HubSwitchGuard !Send to prevent thread corruption” (open, base `master`, head `szokeasaurusrex/hubswitchguard`).
- Description says it fixes GitHub issue #943 and Linear RUST-130.
- Commit message still references GitHub issue #946 and Linear RUST-132; those references now appear out of scope and should be removed/updated to match intent.

## Issue check (in-scope)
### #943 — “HubSwitchGuard should not be Send” (open)
- Repro shows moving a `HubSwitchGuard` across threads corrupts hub state.
- Expected: compile-time error; actual: wrong thread hub is replaced.
- The PR’s `!Send` change in `sentry-core` directly addresses this.

### RUST-130 (Linear)
- Referenced in PR description; not accessible from GitHub CLI, so I can’t verify details.

## Are any parts no longer necessary (given #946 is out of scope)?
- The `sentry-tracing` refactor still appears necessary even if #946 is out of scope. Reason: once `HubSwitchGuard` becomes `!Send`, storing it in span extensions (which can be accessed across threads) risks either compile-time Send/Sync issues or runtime drops on the wrong thread. Moving guards to thread-local storage keeps them on the originating thread and avoids making span data `!Send`.
- What does look unnecessary now: the references to #946 / RUST-132 in the commit message and any PR description text implying that fix; those should be removed or adjusted to avoid claiming a second issue as a goal.

## Files touched (for traceability)
- `CHANGELOG.md`: adds “Unreleased” entry for the breaking change and fix.
- `sentry-core/src/hub_impl.rs`: makes `SwitchGuard`/`HubSwitchGuard` `!Send` using `PhantomData<MutexGuard<'static, ()>>`.
- `sentry-tracing/src/layer.rs`: removes guard from span extensions; stores it in thread-local map keyed by span ID; removes on exit/close.
