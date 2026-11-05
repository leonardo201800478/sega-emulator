//! Rust-owned D3D12 presentation session and worker lifecycle.
//!
//! The worker creates, owns, pumps, and destroys the HWND/D3D12 host on one
//! dedicated thread. Completed frames cross the boundary through FrameQueue;
//! the emulation producer never transfers graphics objects or waits on a GPU
//! fence from inside Scheduler advancement.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use super::D3d12Host;
use crate::{FrameQueue, FrameQueueError, VideoFrame};

/// Errors reported by the native D3D12 presentation session.
#[derive(Debug)]
pub enum D3d12SessionError {
    /// The lossless frame queue rejected a producer/consumer operation.
    Queue(FrameQueueError),
    /// The operating system could not create the presentation worker thread.
    WorkerStart(std::io::Error),
    /// D3D12 host initialization failed on the presentation thread.
    HostInitialization(String),
    /// The startup handshake ended before the host reported readiness.
    StartupChannelClosed,
    /// Presenting a frame failed for a reason other than device loss.
    Presentation(windows::core::Error),
    /// D3D12 reported that its device was removed, reset, or otherwise invalid.
    /// The original HRESULT is preserved; automatic recreation is not yet enabled.
    DeviceLost(windows::core::Error),
    /// The worker thread panicked before returning its result.
    WorkerPanicked,
    /// The native window was closed by the user.
    WindowClosed,
    /// Shutdown occurred while minimized and accepted frames could not be presented.
    PendingFramesCancelled(u64),
}

impl fmt::Display for D3d12SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queue(error) => write!(formatter, "video session queue error: {error}"),
            Self::WorkerStart(error) => {
                write!(
                    formatter,
                    "could not start D3D12 presentation worker: {error}"
                )
            }
            Self::HostInitialization(error) => {
                write!(formatter, "could not initialize D3D12 host: {error}")
            }
            Self::StartupChannelClosed => {
                write!(
                    formatter,
                    "D3D12 presentation startup channel closed unexpectedly"
                )
            }
            Self::Presentation(error) => {
                write!(formatter, "D3D12 frame presentation failed: {error}")
            }
            Self::DeviceLost(error) => {
                write!(
                    formatter,
                    "D3D12 device loss detected; recovery is pending: {error}"
                )
            }
            Self::WorkerPanicked => write!(formatter, "D3D12 presentation worker panicked"),
            Self::WindowClosed => write!(formatter, "native D3D12 presentation window was closed"),
            Self::PendingFramesCancelled(count) => write!(
                formatter,
                "D3D12 session shutdown while minimized; {count} accepted frame(s) were cancelled"
            ),
        }
    }
}

impl std::error::Error for D3d12SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Queue(error) => Some(error),
            Self::WorkerStart(error) => Some(error),
            Self::Presentation(error) | Self::DeviceLost(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FrameQueueError> for D3d12SessionError {
    fn from(error: FrameQueueError) -> Self {
        Self::Queue(error)
    }
}

struct RunningFlagGuard(Arc<AtomicBool>);

impl Drop for RunningFlagGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// A Rust-owned D3D12 window, presentation worker, and lossless frame queue.
///
/// The HWND and all D3D12 objects are created and destroyed on the worker
/// thread. submit() accepts only completed immutable frames and may block when
/// the bounded queue is full. Call it at a completed-frame host boundary, not
/// from Scheduler advancement. GPU waits and message pumping remain isolated
/// from the emulated machine timeline.
pub struct D3d12Session {
    queue: FrameQueue,
    worker: Option<JoinHandle<Result<(), D3d12SessionError>>>,
    running: Arc<AtomicBool>,
    last_presented_sequence: Arc<AtomicU64>,
    cancelled_pending_frames: Arc<AtomicU64>,
}

impl D3d12Session {
    /// Start the native window and D3D12 host on a dedicated presentation thread.
    ///
    /// render_width/render_height define the swap-chain surface. window_width/
    /// window_height define the initial native window size. queue_capacity
    /// controls lossless frame buffering and must be non-zero.
    pub fn start(
        render_width: u32,
        render_height: u32,
        window_width: u32,
        window_height: u32,
        queue_capacity: usize,
    ) -> Result<Self, D3d12SessionError> {
        let queue = FrameQueue::new(queue_capacity)?;
        let worker_queue = queue.clone();

        let running = Arc::new(AtomicBool::new(false));
        let worker_running = Arc::clone(&running);
        let last_presented_sequence = Arc::new(AtomicU64::new(0));
        let worker_last_presented = Arc::clone(&last_presented_sequence);
        let cancelled_pending_frames = Arc::new(AtomicU64::new(0));
        let worker_cancelled_frames = Arc::clone(&cancelled_pending_frames);

        let (startup_sender, startup_receiver) = mpsc::sync_channel::<Result<(), String>>(1);

        let worker = thread::Builder::new()
            .name("sega-d3d12-presentation".to_owned())
            .spawn(move || {
                let mut host = match D3d12Host::initialize_with_window_size(
                    render_width,
                    render_height,
                    window_width,
                    window_height,
                ) {
                    Ok(host) => host,
                    Err(error) => {
                        let message = error.to_string();
                        let _ = startup_sender.send(Err(message.clone()));
                        let _ = worker_queue.close();
                        return Err(D3d12SessionError::HostInitialization(message));
                    }
                };

                worker_running.store(true, Ordering::Release);
                let _running_guard = RunningFlagGuard(Arc::clone(&worker_running));
                if startup_sender.send(Ok(())).is_err() {
                    let _ = worker_queue.close();
                    return Err(D3d12SessionError::StartupChannelClosed);
                }

                let mut in_flight_cancelled = 0_u64;
                let mut pending_frame: Option<VideoFrame> = None;
                let result = (|| {
                    loop {
                        if !host.pump_messages() {
                            return Err(D3d12SessionError::WindowClosed);
                        }

                        if host.is_minimized() {
                            if worker_queue.is_closed()? {
                                let mut cancelled: u64 = match pending_frame.take() {
                                    Some(_) => 1,
                                    None => 0,
                                };
                                while let Some(_frame) = worker_queue.pop_timeout(Duration::ZERO)? {
                                    cancelled = cancelled.saturating_add(1);
                                }
                                in_flight_cancelled = in_flight_cancelled.saturating_add(cancelled);
                                if cancelled > 0 {
                                    return Err(D3d12SessionError::PendingFramesCancelled(
                                        cancelled,
                                    ));
                                }
                                break;
                            }

                            // Do not dequeue while minimized: retain every
                            // accepted frame and let bounded-queue backpressure
                            // apply at the completed-frame submission boundary.
                            thread::sleep(Duration::from_millis(8));
                            continue;
                        }

                        let next_frame = match pending_frame.take() {
                            Some(frame) => Some(frame),
                            None => worker_queue.pop_timeout(Duration::from_millis(8))?,
                        };

                        if let Some(frame) = next_frame {
                            // Recheck after dequeue to close the race where
                            // the user minimizes between the loop's first
                            // IsIconic check and the queue pop.
                            if host.is_minimized() {
                                pending_frame = Some(frame);
                                continue;
                            }

                            if let Err(error) = host.present_frame(&frame) {
                                in_flight_cancelled = in_flight_cancelled.saturating_add(1);
                                return Err(match host.device_removed_reason() {
                                    Err(reason) => D3d12SessionError::DeviceLost(reason),
                                    Ok(()) => D3d12SessionError::Presentation(error),
                                });
                            }
                            worker_last_presented.store(frame.sequence(), Ordering::Release);
                        } else if worker_queue.is_closed()? {
                            break;
                        }
                    }
                    Ok(())
                })();

                // Closing wakes blocked producers. If the host failed or its
                // window closed, count queued frames that can no longer be
                // presented rather than silently reporting them as displayed.
                let _ = worker_queue.close();
                if result.is_err() {
                    let mut cancelled = 0_u64;
                    while let Ok(Some(_frame)) = worker_queue.pop_timeout(Duration::ZERO) {
                        cancelled = cancelled.saturating_add(1);
                    }
                    cancelled = cancelled.saturating_add(in_flight_cancelled);
                    worker_cancelled_frames.store(cancelled, Ordering::Release);
                }
                result
            })
            .map_err(D3d12SessionError::WorkerStart)?;

        match startup_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                queue,
                worker: Some(worker),
                running,
                last_presented_sequence,
                cancelled_pending_frames,
            }),
            Ok(Err(message)) => {
                let _ = worker.join();
                Err(D3d12SessionError::HostInitialization(message))
            }
            Err(_) => {
                let _ = worker.join();
                Err(D3d12SessionError::StartupChannelClosed)
            }
        }
    }

    /// Submit one completed frame to the lossless queue.
    ///
    /// This may wait for queue capacity. It must be called after the machine
    /// has completed the frame and outside Scheduler event advancement.
    pub fn submit(&self, frame: VideoFrame) -> Result<(), D3d12SessionError> {
        self.queue.push(frame).map_err(D3d12SessionError::Queue)
    }

    /// Return whether the native presentation worker is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Return the sequence number of the last frame successfully presented.
    pub fn last_presented_sequence(&self) -> u64 {
        self.last_presented_sequence.load(Ordering::Acquire)
    }

    /// Return how many accepted pending frames were canceled after host failure
    /// or native-window closure.
    pub fn cancelled_pending_frames(&self) -> u64 {
        self.cancelled_pending_frames.load(Ordering::Acquire)
    }

    /// Return the number of frames waiting for presentation.
    pub fn queued_frames(&self) -> Result<usize, D3d12SessionError> {
        self.queue.len().map_err(D3d12SessionError::Queue)
    }

    /// Close the producer side, drain accepted frames, and join the worker.
    ///
    /// A user-closed native window is treated as a normal session termination;
    /// any frames that could no longer be presented are reported by
    /// cancelled_pending_frames().
    pub fn shutdown(mut self) -> Result<(), D3d12SessionError> {
        self.queue.close().map_err(D3d12SessionError::Queue)?;
        self.join_worker()
    }

    fn join_worker(&mut self) -> Result<(), D3d12SessionError> {
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };

        match worker.join() {
            Ok(Ok(())) | Ok(Err(D3d12SessionError::WindowClosed)) => Ok(()),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(D3d12SessionError::WorkerPanicked),
        }
    }
}

impl Drop for D3d12Session {
    fn drop(&mut self) {
        let _ = self.queue.close();
        let _ = self.join_worker();
    }
}
