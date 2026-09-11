---
name: docs-review
description: Narrow docs review for sentry-rust. Flag missed API docs updates after public API changes, and flag inaccurate new or changed documentation. Use for rustdoc, crate README, and public API documentation drift.
allowed-tools: Read Grep Glob
---

# Docs Review

Review the current change for documentation drift in the Sentry Rust SDK. Stay inside this narrow scope.

This repository's user-facing API docs are primarily:

- Rustdoc on public items (`///`, `//!`) that publish to docs.rs
- Crate `README.md` files generated from crate-level rustdoc via `cargo readme` / `scripts/generate-readme.sh`
- The workspace `README.md` when it describes crates or public capabilities

## Scope

Report only these two classes of issues:

1. **Missed docs update** — the change alters user-visible public API or behavior, but docs that should describe it were not updated.
2. **Inaccurate new docs** — the change adds or edits documentation that does not match the current code.

Do not expand into general docs quality, style, completeness, or architecture review.

## What Counts as Docs-Relevant

Treat a change as docs-relevant when it affects a **public** surface users rely on:

- New, removed, renamed, or re-exported public items (`pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub type`, `pub const`, `pub static`, public modules)
- Changed public signatures, generics, lifetimes, or trait bounds
- Changed defaults, feature-flag gating, or builder/option semantics that docs describe
- Changed panic, error, or return behavior that existing docs claim
- New or edited rustdoc / README text

Private helpers, test-only APIs, and internal refactors with no public contract change are out of scope unless the diff itself adds inaccurate docs.

## Review Process

1. Identify what the hunk changes: code, docs, or both.
2. Determine the public contract from signatures, nearby rustdoc, feature gates, and related README text.
3. Apply the matching check below.
4. Cross-check with nearby unchanged docs only as needed to verify accuracy or confirm a miss. Do not audit untouched docs.

### Check A: Missed Docs Update

Report when the code change clearly requires a docs update and the matching docs were not updated in this change set.

Typical misses:

- New public API with no rustdoc (or only a placeholder that does not describe behavior)
- Changed signature, default, feature gate, or behavior while rustdoc still describes the old contract
- Public API added/removed/renamed while crate or workspace README still lists the old surface
- Crate-level rustdoc (`//!`) changed in a way that should regenerate `README.md`, but the generated README was left stale

For README staleness, remember crate READMEs are generated from rustdoc. If `lib.rs` module docs changed and `README.md` still reflects the previous text, report that.

### Check B: Inaccurate New or Changed Docs

Report when added or edited docs are wrong relative to the current code.

Typical inaccuracies:

- Docs describe parameters, return values, defaults, or feature flags that do not match the implementation
- Examples in rustdoc or README do not compile against the current public API, or demonstrate removed/renamed APIs
- Docs claim behavior the code does not implement (or omit a critical restriction the code enforces, such as a required feature flag)
- Links or intra-doc references point at removed items or incorrect paths

## Severity

| Level | Use for |
|-------|---------|
| high | Public docs that are affirmatively wrong about safety-critical behavior, defaults that change telemetry/privacy, or removed APIs still documented as available |
| medium | Clear missed docs update for a public API change, or inaccurate docs that would mislead normal API usage |
| low | Narrow inaccuracy with limited blast radius, or a missed docs touch-up that does not leave the old contract actively wrong |

Use the lower severity when the mismatch depends on unproven assumptions.

## What Not To Report

- Private item docs, internal comments, or test comments
- Style, tone, grammar, formatting, or rustdoc convention nits
- Missing examples, missing `# Errors` / `# Panics` sections, or "docs could be richer" suggestions
- Changelog wording, unless the changed changelog text itself misstates the public API
- Exhaustive coverage audits of unchanged public APIs
- Speculative doc needs for refactors with no public contract change
- Generated/vendored noise unrelated to user-facing API docs

If neither Check A nor Check B is clearly satisfied, return no findings.

## Finding Guidance

- Prefer one finding per concrete docs miss or inaccuracy.
- Anchor the finding to a changed line in the hunk. If the stale docs live elsewhere, say where in the description and still anchor to the changed public API or doc line that proves the mismatch.
- Keep the description short and actionable: what is wrong, and what should be updated.
- Put the evidence trace in `verification` (signatures, doc quotes, feature gates, README paths).
