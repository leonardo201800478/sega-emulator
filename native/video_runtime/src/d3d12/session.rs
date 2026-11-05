//! Rust-owned D3D12 presentation session and worker lifecycle.
//!
//! The worker creates, owns, pumps, and destroys the HWND/D3D12 host on one
//! dedicated thread. Completed frames cross the boundary through FrameQueue;
//! the emulation producer never transfers graphics objects or waits on a GPU
//! fence from inside Scheduler advancement.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
    /// Presenting a frame failed on the presentation thread.
    Presentation(String),
    /// The worker thread panicked before returning its result.
    WorkerPanicked,
    /// The native window was closed by the user.
    WindowClosed,
}

impl fmt::Display for D3d12SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queue(error) => write!(formatter, "video session queue error: {error}"),
            Self::WorkerStart(error) => {
                write!(formatter, "could not start D3D12 presentation worker: {error}")
            }
            Self::HostInitialization(error) => {
                write!(formatter, "could not initialize D3D12 host: {error}")
            }
            Self::StartupChannelClosed => {
                write!(formatter, "D3D12 presentation startup channel closed unexpectedly")
            }
            Self::Presentation(error) => {
                write!(formatter, "D3D12 frame presentation failed: {error}")
            }
            Self::WorkerPanicked => write!(formatter, "D3D12 presentation worker panicked"),
            Self::WindowClosed => write!(formatter, "native D3D12 presentation window was closed"),
        }
    }
}

impl std::error::Error for D3d12SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Queue(error) => Some(error),
            Self::WorkerStart(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FrameQueueError> for D3d12SessionError {
    fn from(error: FrameQueueError) -> Self {
        Self::Queue(error)
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

        let (startup_sender, startup_receiver) =
            mpsc::sync_channel::<Result<(), String>>(1);

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
                if startup_sender.send(Ok(())).is_err() {
                    worker_running.store(false, Ordering::Release);
                    let _ = worker_queue.close();
                    return Err(D3d12SessionError::StartupChannelClosed);
                }

                let result = (|| {
                    loop {
                        if !host.pump_messages() {
                            return Err(D3d12SessionError::WindowClosed);
                        }

                        match worker_queue.pop_timeout(Duration::from_millis(8))? {
                            Some(frame) => {
                                host.present_frame(&frame).map_err(|error| {
                                    D3d12SessionError::Presentation(error.to_string())
                                })?;
                                worker_last_presented
                                    .store(frame.sequence(), Ordering::Release);
                            }
                            None if worker_queue.is_closed()? => break,
                            None => {}
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
                    worker_cancelled_frames.store(cancelled, Ordering::Release);
                }
                worker_running.store(false, Ordering::Release);
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
