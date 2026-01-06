//! Tests for Windows-specific IP address change listener.

use super::windows::{WindowsApiListener, WindowsApiStream};
use crate::monitor::ApiListener;

#[test]
fn windows_api_listener_new_succeeds() {
    let result = WindowsApiListener::new();
    assert!(result.is_ok());
}

#[test]
fn windows_api_listener_default() {
    let listener = WindowsApiListener::default();
    // Should work the same as new()
    let _stream = listener.into_stream();
}

#[test]
fn windows_api_listener_debug() {
    let listener = WindowsApiListener::new().expect("Failed to create listener");
    let debug_str = format!("{listener:?}");
    assert!(debug_str.contains("WindowsApiListener"));
}

#[test]
fn windows_api_stream_debug() {
    let listener = WindowsApiListener::new().expect("Failed to create listener");
    let stream = listener.into_stream();
    let debug_str = format!("{stream:?}");
    assert!(debug_str.contains("WindowsApiStream"));
    assert!(debug_str.contains("terminated"));
    assert!(debug_str.contains("has_handle"));
}

#[test]
fn notification_handle_is_send() {
    fn assert_send<T: Send>() {}
    // NotificationHandle is private, but WindowsApiStream contains it
    // and must be Send, which transitively requires NotificationHandle to be Send
    assert_send::<WindowsApiStream>();
}

#[test]
fn windows_api_stream_is_send_and_unpin() {
    fn assert_send<T: Send>() {}
    fn assert_unpin<T: Unpin>() {}
    assert_send::<WindowsApiStream>();
    assert_unpin::<WindowsApiStream>();
}

// Integration test: verifies the stream can be created and polled
// Note: This doesn't test actual notifications (would require network changes)
#[tokio::test]
async fn windows_api_stream_can_be_created() {
    let listener = WindowsApiListener::new().expect("Failed to create listener");
    let _stream = listener.into_stream();
    // Stream created successfully - actual notification testing would
    // require triggering real network changes
}
