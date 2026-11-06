//! D3D12 presentation session backed by a latest-frame mailbox.
//!
//! The HWND may be owned by the Rust host or borrowed from a GUI toolkit.
//!
//! The producer publishes only completed immutable frames. A newer pending
//! frame may replace an older pending presentation frame; emulation work and
//! Scheduler time are never skipped. The HWND, DXGI objects, GPU waits and
//! Present calls remain on this dedicated presentation thread.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_DEVICE_RESET};

use super::{D3d12Host, NativeVideoWindow};
use crate::{FrameMailbox, FrameMailboxError, VideoFrame};

/// Errors reported by a D3D12 mailbox presentation session.
#[derive(Debug)]
pub enum D3d12MailboxSessionError {
    /// The latest-frame mailbox rejected a producer or consumer operation.
    Mailbox(FrameMailboxError),
    /// The operating system could not create the presentation worker.
    WorkerStart(std::io::Error),
    /// D3D12 host initialization failed on the presentation thread.
    HostInitialization(String),
    /// The startup handshake ended before the host reported readiness.
    StartupChannelClosed,
    /// Presenting a frame failed for a reason other than device loss.
    Presentation(windows::core::Error),
    /// D3D12 reported a non-recoverable device-loss reason.
    DeviceLost(windows::core::Error),
    /// Device recreation or retry of the interrupted frame failed.
    DeviceRecoveryFailed {
        initial_reason: windows::core::Error,
        recovery_error: String,
    },
    /// The presentation worker panicked before returning.
    WorkerPanicked,
    /// The native presentation window was closed by the user.
    WindowClosed,
}

impl fmt::Display for D3d12MailboxSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mailbox(error) => write!(formatter, "video mailbox error: {error}"),
            Self::WorkerStart(error) => {
                write!(formatter, "could not start D3D12 mailbox worker: {error}")
            }
            Self::HostInitialization(error) => {
                write!(formatter, "could not initialize D3D12 host: {error}")
            }
            Self::StartupChannelClosed => write!(
                formatter,
                "D3D12 mailbox startup channel closed unexpectedly"
            ),
            Self::Presentation(error) => {
                write!(formatter, "D3D12 frame presentation failed: {error}")
            }
            Self::DeviceLost(error) => {
                write!(formatter, "non-recoverable D3D12 device loss: {error}")
            }
            Self::DeviceRecoveryFailed {
                initial_reason,
                recovery_error,
            } => write!(
                formatter,
                "D3D12 device recovery failed after {initial_reason}: {recovery_error}"
            ),
            Self::WorkerPanicked => write!(formatter, "D3D12 mailbox worker panicked"),
            Self::WindowClosed => write!(formatter, "native D3D12 presentation window was closed"),
        }
    }
}

impl std::error::Error for D3d12MailboxSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mailbox(error) => Some(error),
            Self::WorkerStart(error) => Some(error),
            Self::Presentation(error) | Self::DeviceLost(error) => Some(error),
            Self::DeviceRecoveryFailed { initial_reason, .. } => Some(initial_reason),
            _ => None,
        }
    }
}

impl From<FrameMailboxError> for D3d12MailboxSessionError {
    fn from(error: FrameMailboxError) -> Self {
        Self::Mailbox(error)
    }
}

fn is_recoverable_device_loss(error: &windows::core::Error) -> bool {
    error.code() == DXGI_ERROR_DEVICE_REMOVED || error.code() == DXGI_ERROR_DEVICE_RESET
}

fn is_window_closed_error(error: &windows::core::Error) -> bool {
    error.code().0 == 0x8000_4004_u32 as i32
}

struct RunningFlagGuard(Arc<AtomicBool>);

impl Drop for RunningFlagGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// A D3D12 presentation session and single-slot latest-frame mailbox.
///
/// Publishing never waits for queue capacity or a GPU fence. It must happen
/// only after a complete frame has been produced, outside Scheduler event
/// advancement. Replaced frames affect presentation only, not machine work.
pub struct D3d12MailboxSession {
    mailbox: FrameMailbox,
    worker: Option<JoinHandle<Result<(), D3d12MailboxSessionError>>>,
    running: Arc<AtomicBool>,
    last_presented_sequence: Arc<AtomicU64>,
    device_recoveries: Arc<AtomicU64>,
}

impl D3d12MailboxSession {
    /// Start the standalone native window and presentation worker.
    pub fn start(
        render_width: u32,
        render_height: u32,
        window_width: u32,
        window_height: u32,
    ) -> Result<Self, D3d12MailboxSessionError> {
        let initialize_host = move || {
            D3d12Host::initialize_with_window_size(
                render_width,
                render_height,
                window_width,
                window_height,
            )
        };
        Self::start_with_host_factory(initialize_host)
    }

    /// Start presentation on an existing HWND owned by the calling GUI thread.
    ///
    /// The caller must invoke this method on the HWND's owning thread and keep
    /// the associated widget alive until shutdown() has completed. The D3D12
    /// worker never pumps or destroys this external window.
    pub fn start_for_existing_window(
        render_width: u32,
        render_height: u32,
        window_handle: usize,
    ) -> Result<Self, D3d12MailboxSessionError> {
        NativeVideoWindow::validate_existing_handle(window_handle)
            .map_err(|error| D3d12MailboxSessionError::HostInitialization(error.to_string()))?;
        let initialize_host =
            move || D3d12Host::initialize_for_existing_window(render_width, render_height, window_handle);
        Self::start_with_host_factory(initialize_host)
    }

    fn start_with_host_factory<F>(
        initialize_host: F,
    ) -> Result<Self, D3d12MailboxSessionError>
    where
        F: Fn() -> windows::core::Result<D3d12Host> + Send + 'static,
    {
        let mailbox = FrameMailbox::new();
        let worker_mailbox = mailbox.clone();
        let running = Arc::new(AtomicBool::new(false));
        let worker_running = Arc::clone(&running);
        let last_presented_sequence = Arc::new(AtomicU64::new(0));
        let worker_last_presented = Arc::clone(&last_presented_sequence);
        let device_recoveries = Arc::new(AtomicU64::new(0));
        let worker_device_recoveries = Arc::clone(&device_recoveries);
        let (startup_sender, startup_receiver) = mpsc::sync_channel::<Result<(), String>>(1);

        let worker = thread::Builder::new()
            .name("sega-d3d12-mailbox-presentation".to_owned())
            .spawn(move || {
                let mut host = match initialize_host() {
                    Ok(host) => host,
                    Err(error) => {
                        let message = error.to_string();
                        let _ = startup_sender.send(Err(message.clone()));
                        let _ = worker_mailbox.close();
                        return Err(D3d12MailboxSessionError::HostInitialization(message));
                    }
                };

                worker_running.store(true, Ordering::Release);
                let _running_guard = RunningFlagGuard(Arc::clone(&worker_running));
                if startup_sender.send(Ok(())).is_err() {
                    let _ = worker_mailbox.close();
                    return Err(D3d12MailboxSessionError::StartupChannelClosed);
                }

                let result = (|| {
                    let mut pending_frame: Option<VideoFrame> = None;
                    loop {
                        if !host.pump_messages() {
                            return Err(D3d12MailboxSessionError::WindowClosed);
                        }

                        if host.is_minimized() {
                            if worker_mailbox.is_closed()? {
                                break;
                            }
                            thread::sleep(Duration::from_millis(8));
                            continue;
                        }

                        let frame = match pending_frame.take() {
                            Some(frame) => Some(frame),
                            None => worker_mailbox.take_latest()?,
                        };

                        let Some(frame) = frame else {
                            if worker_mailbox.is_closed()? {
                                break;
                            }
                            thread::sleep(Duration::from_millis(8));
                            continue;
                        };

                        if host.is_minimized() {
                            pending_frame = Some(frame);
                            continue;
                        }

                        if let Err(error) = host.present_frame(&frame) {
                            let device_reason = host.device_removed_reason().err();
                            let classification_reason = device_reason.as_ref().unwrap_or(&error);

                            if is_recoverable_device_loss(classification_reason) {
                                let initial_reason = device_reason.unwrap_or(error);
                                drop(host);
                                host = match initialize_host() {
                                    Ok(host) => host,
                                    Err(recovery_error) => {
                                        return Err(
                                            D3d12MailboxSessionError::DeviceRecoveryFailed {
                                                initial_reason,
                                                recovery_error: recovery_error.to_string(),
                                            },
                                        );
                                    }
                                };
                                worker_device_recoveries.fetch_add(1, Ordering::AcqRel);

                                if let Err(retry_error) = host.present_frame(&frame) {
                                    let recovery_error = match host.device_removed_reason() {
                                        Err(reason) => {
                                            format!("retry also lost the D3D12 device: {reason}")
                                        }
                                        Ok(()) => retry_error.to_string(),
                                    };
                                    return Err(D3d12MailboxSessionError::DeviceRecoveryFailed {
                                        initial_reason,
                                        recovery_error,
                                    });
                                }
                            } else {
                                return Err(match device_reason {
                                    Some(reason) => D3d12MailboxSessionError::DeviceLost(reason),
                                    None if is_window_closed_error(&error) => {
                                        D3d12MailboxSessionError::WindowClosed
                                    }
                                    None => D3d12MailboxSessionError::Presentation(error),
                                });
                            }
                        }

                        worker_last_presented.store(frame.sequence(), Ordering::Release);
                    }
                    Ok(())
                })();

                let _ = worker_mailbox.close();
                result
            })
            .map_err(D3d12MailboxSessionError::WorkerStart)?;

        match startup_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                mailbox,
                worker: Some(worker),
                running,
                last_presented_sequence,
                device_recoveries,
            }),
            Ok(Err(message)) => {
                let _ = worker.join();
                Err(D3d12MailboxSessionError::HostInitialization(message))
            }
            Err(_) => {
                let _ = worker.join();
                Err(D3d12MailboxSessionError::StartupChannelClosed)
            }
        }
    }

    /// Publish one completed immutable frame; a newer pending frame may replace it.
    pub fn submit(&self, frame: VideoFrame) -> Result<(), D3d12MailboxSessionError> {
        self.mailbox.publish(frame).map_err(Into::into)
    }

    /// Return a cloneable producer handle for a Rust-owned emulation worker.
    pub fn mailbox(&self) -> FrameMailbox {
        self.mailbox.clone()
    }

    /// Return whether the native presentation worker is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Return the sequence number of the last successfully presented frame.
    pub fn last_presented_sequence(&self) -> u64 {
        self.last_presented_sequence.load(Ordering::Acquire)
    }

    /// Return how many pending frames were replaced before presentation.
    pub fn replaced_frames(&self) -> Result<u64, D3d12MailboxSessionError> {
        self.mailbox.replaced_frames().map_err(Into::into)
    }

    /// Return how many successful D3D12 device recreations occurred.
    pub fn device_recovery_count(&self) -> u64 {
        self.device_recoveries.load(Ordering::Acquire)
    }

    /// Close the mailbox and join the worker. Any frame still pending at
    /// shutdown is intentionally not presented; no emulated time is affected.
    pub fn shutdown(mut self) -> Result<(), D3d12MailboxSessionError> {
        self.mailbox.close()?;
        self.join_worker()
    }

    fn join_worker(&mut self) -> Result<(), D3d12MailboxSessionError> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        match worker.join() {
            Ok(Ok(())) | Ok(Err(D3d12MailboxSessionError::WindowClosed)) => Ok(()),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(D3d12MailboxSessionError::WorkerPanicked),
        }
    }
}

impl Drop for D3d12MailboxSession {
    fn drop(&mut self) {
        let _ = self.mailbox.close();
        let _ = self.join_worker();
    }
}


#[cfg(test)]
mod tests {
    use super::{D3d12MailboxSession, D3d12MailboxSessionError};

    #[test]
    fn rejects_zero_external_hwnd_before_starting_worker() {
        let result = D3d12MailboxSession::start_for_existing_window(256, 192, 0);
        assert!(matches!(
            result,
            Err(D3d12MailboxSessionError::HostInitialization(_))
        ));
    }
}
