//! End to end test.
//!
//! The crash event comes from a separate process, so the SDK's in-process
//! `TestTransport` cannot see it. A small HTTP listener stands in for
//! Sentry: it accepts one upload, reads the body, and parses it as an
//! envelope.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, ExitStatus};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use sentry::protocol::{AttachmentType, EnvelopeItem};
use sentry::{Envelope, Level};

/// Reads one HTTP request and returns its body.
fn read_request_body(stream: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];

    // Read until the end of the headers.
    let header_end = loop {
        let n = stream.read(&mut chunk).expect("read request");
        assert!(n > 0, "connection closed before headers");
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos.saturating_add(4);
        }
    };

    let headers = String::from_utf8_lossy(&buf[..header_end]).to_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .map(|v| v.trim().parse::<usize>().expect("content-length"))
        .expect("request has a content-length");

    // Read the rest of the body.
    let body_end = header_end.saturating_add(content_length);
    while buf.len() < body_end {
        let n = stream.read(&mut chunk).expect("read body");
        assert!(n > 0, "connection closed before body end");
        buf.extend_from_slice(&chunk[..n]);
    }

    buf[header_end..body_end].to_vec()
}

#[test]
fn captures_minidump_from_crash() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
    let port = listener
        .local_addr()
        .expect("local address should be available")
        .port();

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept upload");
        let body = read_request_body(&mut stream);
        // Let the reporter's transport finish cleanly.
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
        let _ = tx.send(body);
    });

    // Runs `examples/app.rs`, which crashes on purpose. It has to be a
    // separate binary because the crash reporter re-executes it and the
    // crash event is sent from that second process, not the test process.
    let example_process = Command::new(env!("CARGO"))
        .args(["run", "--quiet", "--example", "minidump"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("SENTRY_DSN", format!("http://dsn@127.0.0.1:{port}/0"))
        .spawn()
        .expect("spawn example");

    // Wait for the example process. As this may involve rebuilding the example, we set the
    // timeout pretty high at 120 seconds.
    wait_for_timeout(example_process, Duration::from_secs(120));

    let body = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("received an envelope");

    let envelope = Envelope::from_slice(&body).expect("parse envelope");

    let event = envelope
        .items()
        .find_map(|item| match item {
            EnvelopeItem::Event(event) => Some(event),
            _ => None,
        })
        .expect("envelope has an event");

    assert_eq!(event.level, Level::Fatal);

    // Set by `before_capture` in the example.
    assert_eq!(
        event.tags.get("crash_reporter").map(String::as_str),
        Some("example")
    );

    // Set through `with_integration` before the crash.
    let user = event.user.as_ref().expect("event has a user");
    assert_eq!(user.username.as_deref(), Some("john_doe"));
    assert_eq!(user.email.as_deref(), Some("john@doe.town"));

    let attachment = envelope
        .items()
        .find_map(|item| match item {
            EnvelopeItem::Attachment(attachment) => Some(attachment),
            _ => None,
        })
        .expect("envelope has an attachment");

    assert_eq!(attachment.ty, Some(AttachmentType::Minidump));
    assert!(
        attachment.buffer.starts_with(b"MDMP"),
        "attachment is a minidump"
    );
}

/// Waits for a process to exit for up to a certain timeout.
///
/// If the process does not exit within the timeout, it is killed and this function panics.
fn wait_for_timeout(mut process: Child, timeout: Duration) -> ExitStatus {
    let deadline = Instant::now()
        .checked_add(timeout)
        .expect("deadline overflowed Instant");

    while Instant::now() < deadline {
        if let Some(status) = process.try_wait().expect("error while waiting on process") {
            return status;
        }

        thread::sleep(Duration::from_millis(100));
    }

    process.kill().expect("error while killing process");
    panic!("Process did not exit within timeout.");
}
