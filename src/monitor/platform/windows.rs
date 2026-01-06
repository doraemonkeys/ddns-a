//! Windows-specific IP address change listener using `NotifyIpInterfaceChange`.

use crate::monitor::{ApiError, ApiListener};
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context, Poll};
use tokio::sync::mpsc as tokio_mpsc;
use tokio_stream::Stream;
use windows::Win32::Foundation::{HANDLE, NO_ERROR, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    CancelMibChangeNotify2, MIB_IPINTERFACE_ROW, MIB_NOTIFICATION_TYPE, NotifyIpInterfaceChange,
};
use windows::Win32::Networking::WinSock::AF_UNSPEC;

/// Windows implementation of [`ApiListener`] using `NotifyIpInterfaceChange`.
///
/// This listener uses the Windows IP Helper API to receive notifications
/// when IP interface changes occur. It converts the callback-based API
/// into an async stream.
///
/// # One-time Semantics
///
/// Once `into_stream` is called, the listener is consumed. If the stream
/// encounters an error, callers should fall back to polling-only mode
/// rather than attempting to recreate the listener.
///
/// # Example
///
/// ```no_run
/// use ddns_a::monitor::platform::WindowsApiListener;
/// use ddns_a::monitor::ApiListener;
/// use tokio_stream::StreamExt;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let listener = WindowsApiListener::new()?;
/// let mut stream = listener.into_stream();
///
/// while let Some(result) = stream.next().await {
///     match result {
///         Ok(()) => println!("IP interface changed"),
///         Err(e) => {
///             eprintln!("Listener error: {e}");
///             break; // Fall back to polling
///         }
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Default)]
pub struct WindowsApiListener {
    // Currently no configuration needed, but struct allows future extension
    _private: (),
}

impl WindowsApiListener {
    /// Creates a new Windows API listener.
    ///
    /// # Errors
    ///
    /// This constructor cannot fail, but returns `Result` for API consistency
    /// and future extensibility.
    pub const fn new() -> Result<Self, ApiError> {
        Ok(Self { _private: () })
    }
}

impl ApiListener for WindowsApiListener {
    type Stream = WindowsApiStream;

    fn into_stream(self) -> Self::Stream {
        WindowsApiStream::new()
    }
}

/// Stream of IP interface change notifications from Windows API.
///
/// This stream wraps the `NotifyIpInterfaceChange` callback mechanism,
/// delivering notifications through a tokio channel.
pub struct WindowsApiStream {
    /// Receiver for notification events
    receiver: tokio_mpsc::UnboundedReceiver<Result<(), ApiError>>,
    /// Handle for cancelling the notification registration.
    /// This field is used implicitly through its `Drop` impl which calls
    /// `CancelMibChangeNotify2` to clean up the Windows notification.
    #[allow(dead_code)]
    handle: Option<NotificationHandle>,
    /// Whether the stream has terminated due to error
    terminated: bool,
}

impl std::fmt::Debug for WindowsApiStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsApiStream")
            .field("terminated", &self.terminated)
            .field("has_handle", &self.handle.is_some())
            .finish_non_exhaustive()
    }
}

/// RAII wrapper for the notification handle.
///
/// Automatically cancels the notification registration when dropped,
/// and reclaims the leaked `CallbackContext` to prevent memory/thread leaks.
struct NotificationHandle {
    handle: HANDLE,
    /// Raw pointer to reclaim the leaked `CallbackContext` after cancellation.
    /// Dropping the context closes the channel, allowing the bridge thread to exit.
    context_ptr: *mut CallbackContext,
}

impl Drop for NotificationHandle {
    fn drop(&mut self) {
        // SAFETY: We own this handle and it was returned by NotifyIpInterfaceChange.
        // CancelMibChangeNotify2 is safe to call once per handle.
        let _ = unsafe { CancelMibChangeNotify2(self.handle) };

        // SAFETY: After CancelMibChangeNotify2 returns, Windows guarantees the
        // callback won't fire again, so we can safely reclaim the context.
        // Dropping the context drops the sender, which closes the channel and
        // allows the bridge thread to exit cleanly.
        drop(unsafe { Box::from_raw(self.context_ptr) });
    }
}

// SAFETY: The HANDLE is thread-safe for the cancel operation.
// The Windows API guarantees that CancelMibChangeNotify2 can be called
// from any thread.
unsafe impl Send for NotificationHandle {}

/// Context passed to the Windows callback.
///
/// Contains the sender half of the channel to deliver notifications.
struct CallbackContext {
    sender: mpsc::Sender<()>,
}

impl WindowsApiStream {
    /// Creates a new Windows API stream.
    ///
    /// Registers for IP interface change notifications using the Windows API.
    fn new() -> Self {
        // Create a sync channel for the callback (called from Windows thread pool)
        let (sync_tx, sync_rx) = mpsc::channel::<()>();

        // Create an async channel for the stream consumer
        let (async_tx, async_rx) = tokio_mpsc::unbounded_channel();

        // Spawn a background task to bridge sync -> async
        // This task will run until the sender is dropped
        let bridge_tx = async_tx.clone();
        std::thread::spawn(move || {
            while sync_rx.recv().is_ok() {
                if bridge_tx.send(Ok(())).is_err() {
                    // Receiver dropped, stop bridging
                    break;
                }
            }
        });

        // Register for notifications
        let (handle, terminated) = match register_notification(sync_tx) {
            Ok((h, ctx_ptr)) => (
                Some(NotificationHandle {
                    handle: h,
                    context_ptr: ctx_ptr,
                }),
                false,
            ),
            Err(e) => {
                // Send the error and mark as terminated
                let _ = async_tx.send(Err(e));
                (None, true)
            }
        };

        Self {
            receiver: async_rx,
            handle,
            terminated,
        }
    }
}

impl Stream for WindowsApiStream {
    type Item = Result<(), ApiError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }

        match Pin::new(&mut self.receiver).poll_recv(cx) {
            Poll::Ready(Some(Ok(()))) => Poll::Ready(Some(Ok(()))),
            Poll::Ready(Some(Err(e))) => {
                self.terminated = true;
                Poll::Ready(Some(Err(e)))
            }
            Poll::Ready(None) => {
                // Channel closed unexpectedly
                self.terminated = true;
                Poll::Ready(Some(Err(ApiError::Stopped)))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Registers for IP interface change notifications.
///
/// Returns both the notification handle and the context pointer, so the caller
/// can store them together and reclaim the context when cancelling.
///
/// # Safety
///
/// This function uses unsafe to call Windows API and manage raw pointers.
/// The callback context is leaked intentionally and must be reclaimed by the
/// caller after calling `CancelMibChangeNotify2`.
///
/// # Coverage Note
///
/// This function is excluded from coverage because:
/// - It requires actual Windows API interaction
/// - Callback testing requires triggering real network changes
#[cfg(not(tarpaulin_include))]
fn register_notification(
    sender: mpsc::Sender<()>,
) -> Result<(HANDLE, *mut CallbackContext), ApiError> {
    // Leak the context so it lives for the lifetime of the notification.
    // The caller is responsible for reclaiming it after cancellation.
    let context_ptr = Box::into_raw(Box::new(CallbackContext { sender }));
    let void_ptr = context_ptr.cast::<std::ffi::c_void>();

    let mut handle = HANDLE::default();

    // SAFETY: We provide valid callback and context. The callback will be called
    // from the Windows thread pool when IP interface changes occur.
    // InitialNotification = false means no callback on registration.
    let result = unsafe {
        NotifyIpInterfaceChange(
            AF_UNSPEC,
            Some(ip_interface_change_callback),
            Some(void_ptr),
            false, // InitialNotification
            &raw mut handle,
        )
    };

    if result != NO_ERROR {
        // Clean up leaked context on error
        // SAFETY: Registration failed, so Windows won't call the callback
        drop(unsafe { Box::from_raw(context_ptr) });
        return Err(windows::core::Error::from(WIN32_ERROR(result.0)).into());
    }

    Ok((handle, context_ptr))
}

/// Callback function for `NotifyIpInterfaceChange`.
///
/// This function is called by Windows when IP interface changes occur.
/// It sends a notification through the channel to wake up the async stream.
///
/// # Safety
///
/// - `caller_context` must be a valid pointer to `CallbackContext`
/// - `row` may be null and is not used
///
/// # Coverage Note
///
/// This function is excluded from coverage because it's only called by Windows.
#[cfg(not(tarpaulin_include))]
unsafe extern "system" fn ip_interface_change_callback(
    caller_context: *const std::ffi::c_void,
    _row: *const MIB_IPINTERFACE_ROW,
    _notification_type: MIB_NOTIFICATION_TYPE,
) {
    // SAFETY: caller_context was set by us in register_notification
    // and points to a valid CallbackContext.
    if caller_context.is_null() {
        return;
    }

    let context = unsafe { &*(caller_context.cast::<CallbackContext>()) };

    // Send notification through the channel (ignore send errors - receiver may be dropped)
    let _ = context.sender.send(());
}
