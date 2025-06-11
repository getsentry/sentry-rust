# Sentry Rust SDK Debug Logging Improvements

This document describes the comprehensive debug logging improvements made to the Sentry Rust SDK to address [issue #620](https://github.com/getsentry/sentry-rust/issues/620) - "Improve the ability to debug sentry integration".

## Overview

The Sentry Rust SDK now provides extensive debug logging throughout all major components to help developers understand what's happening during Sentry integration. All debug logging follows a consistent pattern and only appears when `debug: true` is set in the client options.

## How to Enable Debug Logging

```rust
let _guard = sentry::init(sentry::ClientOptions {
    dsn: Some("https://key@sentry.io/42".parse().unwrap()),
    debug: true, // Enable debug logging
    ..Default::default()
});
```

When enabled, debug messages will be printed to stderr with the format: `[sentry] [Component] message`

## Enhanced Components

### 1. Client Initialization (`sentry/src/init.rs`)

The initialization process now provides detailed logging about:
- Configuration parameters (DSN, debug mode, sample rates, release, environment)
- Integration setup and counts
- Transport creation
- Session tracking configuration

**Example output:**
```
[sentry] [init] Initializing Sentry SDK
[sentry] [init] DSN: https://key@sentry.io/42
[sentry] [init] Debug mode: true
[sentry] [init] Sample rate: 1.0
[sentry] [init] Setting up 5 integrations
[sentry] [init] Enabled sentry client for DSN https://key@sentry.io/42
```

### 2. Client Operations (`sentry-core/src/client.rs`)

Enhanced logging covers:
- Client creation and configuration
- Event preparation and processing
- Integration pipeline execution
- Transport operations
- Session and log handling
- Sampling decisions

**Example output:**
```
[sentry] [Client] Creating new client with options: debug=true, dsn=Some("https://key@sentry.io/42")
[sentry] [Client] Setting up 5 integrations
[sentry] [Client] Setting up integration: debug-images
[sentry] [Client] Preparing event a1b2c3d4-e5f6-7890-abcd-ef1234567890 for transmission
[sentry] [Client] Processing event through 5 integrations
[sentry] [Client] Applied client defaults to event fields: release, environment
```

### 3. Scope Management (`sentry-core/src/scope/real.rs`)

Comprehensive logging for scope operations:
- Setting and updating user information
- Managing tags, contexts, and extra data
- Breadcrumb operations
- Event and transaction processing
- Attachment handling

**Example output:**
```
[sentry] [Scope] Setting user: id=Some("user123"), username=Some("john_doe"), email=Some("john@example.com")
[sentry] [Scope] Setting tag: environment = production
[sentry] [Scope] Setting context: device
[sentry] [Scope] Applying scope to event a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [Scope] Applied 3 tags to event
[sentry] [Scope] Applied 2 contexts to event
```

### 4. Transport Layer (`sentry/src/transports/ureq.rs`)

Detailed transport operations logging:
- Transport creation and configuration
- Proxy setup
- Request/response handling
- Rate limiting
- Error conditions

**Example output:**
```
[sentry] [UreqHttpTransport] Creating new ureq transport
[sentry] [UreqHttpTransport] Setting up transport for DSN scheme: Https
[sentry] [UreqHttpTransport] Target URL: https://sentry.io/api/42/envelope/
[sentry] [UreqHttpTransport] Sending envelope to Sentry
[sentry] [UreqHttpTransport] Envelope serialized, size: 1024 bytes
[sentry] [UreqHttpTransport] Received response with status: 200
[sentry] [UreqHttpTransport] Get response: {"id":"a1b2c3d4-e5f6-7890-abcd-ef1234567890"}
```

### 5. Session Management (`sentry-core/src/session.rs`)

Complete session lifecycle tracking:
- Session creation and initialization
- Error updates
- Status transitions
- Background flushing
- Aggregation in request mode

**Example output:**
```
[sentry] [Session] Creating new session from stack
[sentry] [Session] Session created with ID: a1b2c3d4-e5f6-7890-abcd-ef1234567890, distinct_id: Some("user123")
[sentry] [Session] Updated session a1b2c3d4-e5f6-7890-abcd-ef1234567890 due to error event (total errors: 1)
[sentry] [SessionFlusher] Creating new session flusher with mode: Application
[sentry] [SessionFlusher] Enqueueing session update: a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [SessionFlusher] Flushing 1 individual sessions
```

### 6. Logs Batching (`sentry-core/src/logs.rs`)

The existing LogsBatcher already had good logging following the pattern:
```
[sentry] [LogsBatcher] Flushing 5 logs
```

### 7. Integration Processing (`sentry-debug-images/src/integration.rs`)

Enhanced integration logging shows:
- Integration setup
- Event filtering decisions
- Data attachment operations

**Example output:**
```
[sentry] [DebugImagesIntegration] Creating new debug images integration
[sentry] [DebugImagesIntegration] Loading debug images
[sentry] [DebugImagesIntegration] Loaded 42 debug images
[sentry] [DebugImagesIntegration] Adding debug images to event a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [DebugImagesIntegration] Added 42 debug images to event a1b2c3d4-e5f6-7890-abcd-ef1234567890
```

## Consistent Logging Pattern

All debug messages follow the pattern: `[ComponentName] description with relevant details`

- **Client operations**: `[Client] ...`
- **Scope operations**: `[Scope] ...`
- **Transport operations**: `[TransportType] ...` (e.g., `[UreqHttpTransport]`)
- **Session operations**: `[Session]` or `[SessionFlusher] ...`
- **Integration operations**: `[IntegrationName] ...` (e.g., `[DebugImagesIntegration]`)
- **Initialization**: `[init] ...`

## Performance Considerations

- Debug logging uses the `sentry_debug!` macro which compiles to a no-op when debug mode is disabled
- No performance impact in production when `debug: false` (the default)
- Logging only occurs when explicitly enabled via client options
- String formatting only happens when debug mode is active

## Benefits for Developers

### Before (limited debugging):
```
[sentry] Get response: `{"id":"ae0962de93bb4e4bbafa958fb4737a44"}`
```

### After (comprehensive debugging):
```
[sentry] [init] Initializing Sentry SDK
[sentry] [init] DSN: https://key@sentry.io/42
[sentry] [init] Debug mode: true
[sentry] [Client] Creating new client with options: debug=true
[sentry] [Client] Setting up 5 integrations
[sentry] [Client] Setting up integration: debug-images
[sentry] [Client] Transport created successfully
[sentry] [Client] Capturing event: a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [Client] Preparing event a1b2c3d4-e5f6-7890-abcd-ef1234567890 for transmission
[sentry] [Scope] Applying scope to event a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [Client] Processing event through 5 integrations
[sentry] [DebugImagesIntegration] Adding debug images to event a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [Client] Created envelope for event a1b2c3d4-e5f6-7890-abcd-ef1234567890
[sentry] [UreqHttpTransport] Queueing envelope for sending
[sentry] [UreqHttpTransport] Sending envelope to Sentry
[sentry] [UreqHttpTransport] Envelope serialized, size: 2048 bytes
[sentry] [UreqHttpTransport] Received response with status: 200
[sentry] [UreqHttpTransport] Get response: `{"id":"a1b2c3d4-e5f6-7890-abcd-ef1234567890"}`
```

Now developers can:
1. **Track event lifecycle**: See exactly how events are processed from creation to transmission
2. **Debug integration issues**: Understand which integrations are running and when they process events
3. **Monitor transport behavior**: See connection details, request/response cycles, and error conditions
4. **Understand scope operations**: Track how user data, tags, and contexts are applied
5. **Diagnose session problems**: Monitor session creation, updates, and flushing
6. **Correlate events**: Use event IDs to trace events through the entire pipeline

## Implementation Details

- Uses the existing `sentry_debug!` macro for consistency
- Preserves all existing functionality
- Adds zero overhead when debug mode is disabled
- Follows Rust SDK coding guidelines and patterns
- Maintains backward compatibility

This comprehensive logging enhancement significantly improves the debuggability of Sentry integrations, making it much easier for developers to understand and troubleshoot issues with their Sentry setup.