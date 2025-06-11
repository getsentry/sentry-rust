# Sentry Rust SDK Debug Logging Improvements

This document summarizes the comprehensive debug logging improvements made to the Sentry Rust SDK to address GitHub issue #620: "Improve the ability to debug sentry integration".

## Problem Statement

GitHub issue #620 highlighted that users only received minimal debug output like `[sentry] Get response: {"id":"ae0962de93bb4e4bbafa958fb4737a44"}` when enabling debug mode, making it extremely difficult to troubleshoot Sentry integration issues and understand the SDK's internal behavior.

## Solution Overview

The solution adds targeted debug logging at important points throughout the Sentry Rust SDK using the existing `sentry_debug!` macro. The logging follows a consistent pattern `[ComponentName] description with details` and has zero performance impact when debug mode is disabled.

## Key Improvements Made

### 1. Integration Initialization Logging (`sentry/src/defaults.rs`)

Added debug logging for default integration setup:
```rust
sentry_debug!("[apply_defaults] Adding default integrations");
sentry_debug!("[apply_defaults] Adding AttachStacktraceIntegration");
sentry_debug!("[apply_defaults] Adding DebugImagesIntegration");
sentry_debug!("[apply_defaults] Adding ContextIntegration");
sentry_debug!("[apply_defaults] Adding PanicIntegration");
sentry_debug!("[apply_defaults] Adding ProcessStacktraceIntegration");
sentry_debug!("[apply_defaults] Total integrations configured: {}", opts.integrations.len());
```

### 2. Client Operations (`sentry-core/src/client.rs`)

Enhanced client initialization and operations with comprehensive logging:
- Client creation with configuration details
- Integration setup tracking (already present)
- Event preparation and processing pipeline
- Transport operations and rate limiting (already present)
- Session and log handling (already present)

### 3. Integration Processing

#### Context Integration (`sentry-contexts/src/integration.rs`)
```rust
sentry_debug!("[ContextIntegration] Setting up contexts integration");
sentry_debug!("[ContextIntegration] Processing event {}", event.event_id);
sentry_debug!("[ContextIntegration] Added contexts: {}", contexts_added.join(", "));
```

#### Backtrace Integrations (`sentry-backtrace/src/integration.rs`)
```rust
sentry_debug!("[ProcessStacktraceIntegration] Processing event {}", event.event_id);
sentry_debug!("[ProcessStacktraceIntegration] Processed {} stacktraces", processed_stacks);
sentry_debug!("[AttachStacktraceIntegration] Event has no stacktrace, attaching current thread stacktrace");
```

#### Panic Integration (`sentry-panic/src/lib.rs`)
```rust
sentry_debug!("[PanicIntegration] Setting up panic handler");
sentry_debug!("[PanicIntegration] Panic detected: {}", message_from_panic_info(info));
sentry_debug!("[PanicIntegration] Created event {} for panic", event.event_id);
```

#### Debug Images Integration (`sentry-debug-images/src/integration.rs`)
Already had comprehensive debug logging implemented.

### 4. Hub Operations (`sentry-core/src/hub.rs` and `sentry-core/src/hub_impl.rs`)

Added logging for key hub operations:
```rust
sentry_debug!("[Hub] Capturing event: {}", event.event_id);
sentry_debug!("[Hub] Starting new session");
sentry_debug!("[Hub] Binding client to hub (client: {})", if has_client { "present" } else { "none" });
sentry_debug!("[Hub] Adding breadcrumb to scope");
```

### 5. Scope Operations (`sentry-core/src/scope/real.rs`)

Added targeted logging for scope changes:
```rust
sentry_debug!("[Scope] Setting level override: {:?}", new_level);
sentry_debug!("[Scope] Setting user: id={:?}, username={:?}, email={:?}", ...);
sentry_debug!("[Scope] Setting transaction: {}", new_tx);
sentry_debug!("[Scope] Applying scope to event {}", event.event_id);
sentry_debug!("[Scope] Added attachment: {} (total: {})", filename, self.attachments.len());
```

### 6. Logging Integration (`sentry-log/src/logger.rs`)

Enhanced SentryLogger with debug tracking:
```rust
sentry_debug!("[SentryLogger] Creating new SentryLogger");
sentry_debug!("[SentryLogger] Filter result for {} log: {:?}", record.level(), filter_result);
sentry_debug!("[SentryLogger] Capturing event from {} log: {}", record.level(), e.event_id);
```

### 7. Tracing Integration (`sentry-tracing/src/layer.rs`)

Added comprehensive tracing layer logging:
```rust
sentry_debug!("[SentryLayer] Creating default SentryLayer");
sentry_debug!("[SentryLayer] Processing tracing event at {} level, filter result: {:?}", ...);
sentry_debug!("[SentryLayer] Creating new Sentry span for tracing span: {}", span_name);
sentry_debug!("[SentryLayer] Starting new transaction: {}", description);
```

### 8. Transport Operations

Transport implementations already had comprehensive debug logging including:
- Connection setup and configuration
- Request/response handling
- Rate limiting and retry logic
- Error conditions

### 9. Session Management

Session handling already had comprehensive debug logging including:
- Session creation and lifecycle
- Aggregation and flushing
- Background worker operations

## Usage

To enable debug logging, set `debug: true` in your `ClientOptions`:

```rust
let _guard = sentry::init(sentry::ClientOptions {
    dsn: Some("your-dsn-here".parse().unwrap()),
    debug: true,
    ..Default::default()
});
```

## Example Debug Output

Before these improvements, users saw minimal output like:
```
[sentry] Get response: {"id":"ae0962de93bb4e4bbafa958fb4737a44"}
```

After improvements, users now see comprehensive event lifecycle tracking:
```
[apply_defaults] Adding default integrations
[apply_defaults] Adding AttachStacktraceIntegration
[apply_defaults] Adding DebugImagesIntegration
[apply_defaults] Adding ContextIntegration
[apply_defaults] Adding PanicIntegration
[apply_defaults] Adding ProcessStacktraceIntegration
[apply_defaults] Total integrations configured: 5
[Client] Creating new client with options: debug=true, dsn=Some("https://...")
[Client] Setting up 5 integrations
[Client] Setting up integration: attach-stacktrace
[Client] Setting up integration: debug-images
[ContextIntegration] Setting up contexts integration
[Client] Setting up integration: contexts
[PanicIntegration] Setting up panic handler
[Client] Setting up integration: panic
[Client] Setting up integration: process-stacktrace
[Hub] Capturing event: a1b2c3d4-e5f6-7890-abcd-1234567890ab
[Scope] Applying scope to event a1b2c3d4-e5f6-7890-abcd-1234567890ab
[Client] Processing event a1b2c3d4-e5f6-7890-abcd-1234567890ab through 5 integrations
[ContextIntegration] Processing event a1b2c3d4-e5f6-7890-abcd-1234567890ab
[ContextIntegration] Added contexts: os, rust, device
[AttachStacktraceIntegration] Processing event a1b2c3d4-e5f6-7890-abcd-1234567890ab
[AttachStacktraceIntegration] Event has no stacktrace, attaching current thread stacktrace
[UreqHttpTransport] Sending envelope to Sentry
[UreqHttpTransport] Received response with status: 200
[UreqHttpTransport] Get response: {"id":"a1b2c3d4-e5f6-7890-abcd-1234567890ab"}
```

## Technical Implementation

- **Zero Performance Impact**: All logging uses the existing `sentry_debug!` macro which compiles to a no-op when `debug: false`
- **Consistent Naming**: All log messages follow the pattern `[ComponentName] description with details`
- **Event Correlation**: Includes relevant identifiers (event IDs, session IDs) for tracking events through the pipeline
- **Backward Compatibility**: No breaking changes to existing APIs
- **Compilation**: All changes compile successfully with only deprecation warnings about trailing semicolons in macros

## Files Modified

1. `sentry/src/defaults.rs` - Integration initialization logging
2. `sentry-core/src/client.rs` - Enhanced client operations (some already present)
3. `sentry-core/src/hub.rs` - Hub operations logging
4. `sentry-core/src/hub_impl.rs` - Hub implementation logging
5. `sentry-core/src/scope/real.rs` - Scope operations logging
6. `sentry-contexts/src/integration.rs` - Context integration logging
7. `sentry-backtrace/src/integration.rs` - Backtrace integration logging
8. `sentry-panic/src/lib.rs` - Panic integration logging
9. `sentry-log/src/logger.rs` - Log integration logging
10. `sentry-tracing/src/layer.rs` - Tracing integration logging
11. Transport files already had comprehensive logging
12. Session management already had comprehensive logging

## Impact

These improvements transform the Sentry Rust SDK debugging experience from minimal, cryptic output to comprehensive event lifecycle tracking. Developers can now:

1. **Verify Integration Setup**: See exactly which integrations are being loaded and configured
2. **Track Event Processing**: Follow events through the entire pipeline from creation to transmission
3. **Debug Configuration Issues**: Understand when and why events are being filtered, modified, or dropped
4. **Monitor Performance**: See timing and batching information for sessions and logs
5. **Troubleshoot Transport**: Get detailed information about HTTP requests, responses, and rate limiting

This addresses the core issue raised in GitHub #620 and significantly improves the developer experience when integrating and troubleshooting Sentry in Rust applications.