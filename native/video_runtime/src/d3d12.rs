//! Native Direct3D 12 presentation host for Windows x64.
//!
//! This milestone manages the Win32 presentation HWND (owned or borrowed), DXGI factory/swap chain,
//! D3D12 device, direct queue, command allocator/list, render-target views and
//! fence lifecycle. It uploads immutable VideoFrame data from the presentation
//! mailbox, converts RGB565/RGB888 to RGBA8888 when required, and presents through
//! DXGI. The synthetic color-bar path remains available as a host diagnostic.
//!
//! Window resize/DPI handling is owned by Win32/DXGI. Device-loss recovery and
//! physical-GPU validation remain pending. GPU waits use finite polling on the
//! presentation thread and must never gate emulation or Scheduler advancement.

use core::ffi::c_void;
use core::fmt;
use core::mem::ManuallyDrop;
use core::ptr::null_mut;
use std::marker::PhantomData;
use std::rc::Rc;
use std::slice;

use super::{FrameMailbox, FrameMailboxError, PixelFormat, VideoFrame};

use windows::Win32::Foundation::{
    CloseHandle, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HANDLE, HINSTANCE, HWND, LPARAM,
    LRESULT, RECT, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM,
};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_COMMAND_QUEUE_FLAG_NONE,
    D3D12_COMMAND_QUEUE_PRIORITY_NORMAL, D3D12_CPU_DESCRIPTOR_HANDLE,
    D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_DESCRIPTOR_HEAP_DESC, D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
    D3D12_DESCRIPTOR_HEAP_TYPE_RTV, D3D12_FENCE_FLAG_NONE, D3D12_HEAP_FLAG_NONE,
    D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_HEAP_TYPE_UPLOAD,
    D3D12_MEMORY_POOL_UNKNOWN, D3D12_PLACED_SUBRESOURCE_FOOTPRINT, D3D12_RANGE,
    D3D12_RESOURCE_BARRIER, D3D12_RESOURCE_BARRIER_0, D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
    D3D12_RESOURCE_BARRIER_FLAG_NONE, D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, D3D12_RESOURCE_DESC,
    D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_DIMENSION_TEXTURE2D, D3D12_RESOURCE_FLAG_NONE,
    D3D12_RESOURCE_STATE_COPY_DEST, D3D12_RESOURCE_STATE_COPY_SOURCE,
    D3D12_RESOURCE_STATE_GENERIC_READ, D3D12_RESOURCE_STATE_PRESENT,
    D3D12_RESOURCE_STATE_RENDER_TARGET, D3D12_RESOURCE_TRANSITION_BARRIER,
    D3D12_TEXTURE_COPY_LOCATION, D3D12_TEXTURE_COPY_LOCATION_0,
    D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
    D3D12_TEXTURE_LAYOUT_ROW_MAJOR, D3D12_TEXTURE_LAYOUT_UNKNOWN, D3D12CreateDevice,
    ID3D12CommandAllocator, ID3D12CommandList, ID3D12CommandQueue, ID3D12DescriptorHeap,
    ID3D12Device, ID3D12Fence, ID3D12GraphicsCommandList, ID3D12PipelineState, ID3D12Resource,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
    DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIFactory2, IDXGISwapChain2,
    IDXGISwapChain3,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{CreateEventW, GetCurrentThreadId, WaitForSingleObjectEx};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, EnableNonClientDpiScaling,
    SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GA_ROOT, GWL_STYLE, GetAncestor, GetWindowLongPtrW, GetWindowThreadProcessId,
    IsIconic, IsWindow, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SW_SHOW, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_DESTROY,
    WM_DPICHANGED, WM_NCCREATE, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{Error, Interface, Result, w};

mod mailbox_session;
mod session;
pub use mailbox_session::{D3d12MailboxSession, D3d12MailboxSessionError};
pub use session::{D3d12Session, D3d12SessionError};

const SWAP_CHAIN_BUFFER_COUNT: u32 = 2;
const PRESENTATION_WAIT_POLL_MS: u32 = 16;

struct ThreadDpiAwarenessGuard(DPI_AWARENESS_CONTEXT);

impl Drop for ThreadDpiAwarenessGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetThreadDpiAwarenessContext(self.0);
        }
    }
}

/// Native HWND wrapper; the host tracks whether it owns or borrows the window.
pub struct NativeVideoWindow {
    hwnd: HWND,
    owns_window: bool,
}

impl NativeVideoWindow {
    /// Create and show the native presentation window.
    pub fn create(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "window dimensions must be in the non-zero i32 range",
            ));
        }

        // Assign DPI awareness to this HWND only. Restore the worker thread's
        // prior context immediately after CreateWindowExW so the host does not
        // mutate the process-wide or long-lived thread policy.
        let previous_context = unsafe {
            let per_monitor_v2 =
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
            if per_monitor_v2.is_invalid() {
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE)
            } else {
                per_monitor_v2
            }
        };
        if previous_context.is_invalid() {
            return Err(Error::from_thread());
        }
        let dpi_guard = ThreadDpiAwarenessGuard(previous_context);

        let instance = unsafe { GetModuleHandleW(None)? };
        let class_name = w!("SegaEmulatorD3D12Window");
        register_d3d12_window_class(HINSTANCE(instance.0))?;

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("Sega Emulator - Direct3D 12"),
                WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                width as i32,
                height as i32,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )?
        };

        drop(dpi_guard);

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        Ok(Self {
            hwnd,
            owns_window: true,
        })
    }

    /// Validate an external HWND and require its owning GUI thread.
    ///
    /// D3D12 session startup must be invoked on the HWND owner thread. The
    /// presentation worker borrows the handle but never pumps or destroys it.
    pub(crate) fn validate_existing_handle(handle: usize) -> Result<()> {
        if handle == 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "external HWND must be non-zero",
            ));
        }
        let hwnd = HWND(handle as *mut c_void);
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "external HWND is not a valid window",
            ));
        }

        let owner_thread = unsafe { GetWindowThreadProcessId(hwnd, None) };
        let current_thread = unsafe { GetCurrentThreadId() };
        if owner_thread == 0 || owner_thread != current_thread {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "external HWND must be attached from its owning GUI thread",
            ));
        }
        Ok(())
    }

    /// Wrap an externally owned HWND without taking ownership or destroying it.
    ///
    /// The caller must validate the handle on its owning thread and keep the
    /// native window alive until the D3D12 session has shut down.
    fn from_existing_handle(handle: usize) -> Self {
        Self {
            hwnd: HWND(handle as *mut c_void),
            owns_window: false,
        }
    }

    /// Return the native HWND without transferring its ownership.
    pub const fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Return whether Windows currently considers this window minimized.
    ///
    /// The caller must avoid dequeuing completed frames while minimized so
    /// presentation suspension cannot silently discard accepted frames.
    pub fn is_minimized(&self) -> bool {
        if self.owns_window {
            unsafe { IsIconic(self.hwnd).as_bool() }
        } else {
            let root = unsafe { GetAncestor(self.hwnd, GA_ROOT) };
            !root.0.is_null() && unsafe { IsIconic(root).as_bool() }
        }
    }

    /// Pump pending Win32 messages; return false after WM_QUIT.
    pub fn pump_messages(&self) -> bool {
        if !self.owns_window {
            // The parent GUI thread owns and pumps messages for this HWND.
            return true;
        }
        let mut message = MSG::default();
        let mut running = true;

        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                if message.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    running = false;
                    break;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        running && unsafe { IsWindow(Some(self.hwnd)).as_bool() }
    }
}

/// Rust-owned child HWND used as a native D3D12 viewport inside a GUI window.
///
/// This wrapper is deliberately thread-affine: create, resize, and drop it on
/// the parent GUI thread. The D3D12 presentation session receives only the
/// raw handle and borrows it from its dedicated worker thread.
pub struct D3d12ChildWindow {
    hwnd: HWND,
    parent_hwnd: HWND,
    added_parent_clip_children: bool,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl D3d12ChildWindow {
    /// Create a visible child viewport under an existing GUI-owned parent HWND.
    ///
    /// The parent receives WS_CLIPCHILDREN while the viewport exists so its
    /// client painting does not overwrite the independently presented child.
    pub fn create(parent_handle: usize) -> Result<Self> {
        NativeVideoWindow::validate_existing_handle(parent_handle)?;
        let instance = unsafe { GetModuleHandleW(None)? };
        register_d3d12_window_class(HINSTANCE(instance.0))?;

        let parent = HWND(parent_handle as *mut c_void);
        let parent_style = unsafe { GetWindowLongPtrW(parent, GWL_STYLE) };
        let added_parent_clip_children = parent_style & (WS_CLIPCHILDREN.0 as isize) == 0;
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("SegaEmulatorD3D12Window"),
                w!(""),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPSIBLINGS.0),
                0,
                0,
                1,
                1,
                Some(parent),
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )?
        };

        if added_parent_clip_children {
            let updated_style = parent_style | WS_CLIPCHILDREN.0 as isize;
            unsafe {
                SetWindowLongPtrW(parent, GWL_STYLE, updated_style);
            }
            let applied_style = unsafe { GetWindowLongPtrW(parent, GWL_STYLE) };
            if applied_style & (WS_CLIPCHILDREN.0 as isize) == 0 {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
                return Err(Error::new(
                    windows::core::HRESULT(0x8000_4005_u32 as i32),
                    "could not enable WS_CLIPCHILDREN on the eframe parent HWND",
                ));
            }

            if let Err(error) = unsafe {
                SetWindowPos(
                    parent,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                )
            } {
                unsafe {
                    SetWindowLongPtrW(parent, GWL_STYLE, parent_style);
                    let _ = DestroyWindow(hwnd);
                }
                return Err(error);
            }
        }

        sega_observability::tracing::debug!(
            target: "sega::video::d3d12",
            parent_hwnd = parent_handle,
            child_hwnd = hwnd.0 as usize,
            added_parent_clip_children,
            "D3D12 child presentation window created"
        );
        Ok(Self {
            hwnd,
            parent_hwnd: parent,
            added_parent_clip_children,
            _thread_affinity: PhantomData,
        })
    }

    /// Return the child HWND without transferring ownership.
    pub fn handle(&self) -> usize {
        self.hwnd.0 as usize
    }

    /// Move and resize the child viewport in parent-client physical pixels.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        if x < 0 || y < 0 || width <= 0 || height <= 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "child viewport bounds require non-negative origin and positive dimensions",
            ));
        }

        sega_observability::tracing::trace!(
            target: "sega::video::d3d12",
            child_hwnd = self.hwnd.0 as usize,
            x,
            y,
            width,
            height,
            "D3D12 child viewport bounds updated"
        );
        if let Err(error) = unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
            )
        } {
            sega_observability::tracing::error!(
                target: "sega::video::d3d12",
                child_hwnd = self.hwnd.0 as usize,
                x,
                y,
                width,
                height,
                error = %error,
                "D3D12 child viewport positioning failed"
            );
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for D3d12ChildWindow {
    fn drop(&mut self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            self.hwnd = HWND::default();
        }

        if self.added_parent_clip_children && unsafe { IsWindow(Some(self.parent_hwnd)).as_bool() }
        {
            unsafe {
                let current_style = GetWindowLongPtrW(self.parent_hwnd, GWL_STYLE);
                SetWindowLongPtrW(
                    self.parent_hwnd,
                    GWL_STYLE,
                    current_style & !(WS_CLIPCHILDREN.0 as isize),
                );
                let _ = SetWindowPos(
                    self.parent_hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
        }
        self.parent_hwnd = HWND::default();
    }
}

fn register_d3d12_window_class(instance: HINSTANCE) -> Result<()> {
    let class_name = w!("SegaEmulatorD3D12Window");
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: class_name,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        // The process-wide class remains registered after its HWND is destroyed.
        let last_error = unsafe { GetLastError() };
        if last_error != ERROR_CLASS_ALREADY_EXISTS {
            return Err(Error::from_thread());
        }
    }
    Ok(())
}

impl Drop for NativeVideoWindow {
    fn drop(&mut self) {
        if self.owns_window && !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
        self.hwnd = HWND::default();
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // PMv2 scales non-client areas automatically. This call additionally
        // enables that behavior for the PMv1 fallback context. Child viewports
        // have no non-client area; the return value is intentionally ignored.
        let _ = unsafe { EnableNonClientDpiScaling(hwnd) };
    }

    let is_root_window = unsafe { GetAncestor(hwnd, GA_ROOT) == hwnd };
    if message == WM_DPICHANGED && is_root_window && lparam.0 != 0 {
        // Windows supplies the suggested outer-window rectangle for the new
        // monitor DPI. The swap-chain's logical SMS surface remains unchanged;
        // DXGI stretch scaling continues to map it to the resized client area.
        let suggested = unsafe { &*(lparam.0 as *const RECT) };
        let width = suggested.right - suggested.left;
        let height = suggested.bottom - suggested.top;
        let resized = unsafe {
            SetWindowPos(
                hwnd,
                None,
                suggested.left,
                suggested.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        if resized.is_ok() {
            return LRESULT(0);
        }
    }

    if message == WM_DESTROY {
        // Do not post WM_QUIT here: a device-loss recovery can destroy and
        // recreate this HWND on the same worker thread. pump_messages() checks
        // whether its own HWND still exists to detect user closure.
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn transition_barrier(
    resource: &ID3D12Resource,
    before: windows::Win32::Graphics::Direct3D12::D3D12_RESOURCE_STATES,
    after: windows::Win32::Graphics::Direct3D12::D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: ManuallyDrop::new(Some(resource.clone())),
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
                StateBefore: before,
                StateAfter: after,
            }),
        },
    }
}

/// Error returned when consuming a frame mailbox for D3D12 presentation.
#[derive(Debug)]
pub enum MailboxPresentError {
    /// The frame mailbox rejected the consumer operation.
    Mailbox(FrameMailboxError),
    /// The D3D12 upload or presentation operation failed.
    Graphics(windows::core::Error),
}

impl fmt::Display for MailboxPresentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mailbox(error) => write!(formatter, "frame mailbox error: {error}"),
            Self::Graphics(error) => write!(formatter, "D3D12 presentation error: {error}"),
        }
    }
}

impl std::error::Error for MailboxPresentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mailbox(error) => Some(error),
            Self::Graphics(error) => Some(error),
        }
    }
}

/// Direct3D 12/DXGI host with one reusable command allocator/list.
///
/// The single-allocator policy deliberately waits for the previous GPU fence
/// before reuse. This first renderer favors explicit correctness over overlap;
/// a later measured optimization can introduce per-frame contexts without
/// changing the emulated machine timeline.
pub struct D3d12Host {
    _factory: IDXGIFactory2,
    _device: ID3D12Device,
    queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    swap_chain2: IDXGISwapChain2,
    frame_latency_handle: HANDLE,
    rtv_heap: ID3D12DescriptorHeap,
    render_targets: Vec<ID3D12Resource>,
    rtv_handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE>,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
    fence: ID3D12Fence,
    fence_event: HANDLE,
    last_submitted_fence: u64,
    frame_texture: ID3D12Resource,
    upload_buffer: ID3D12Resource,
    upload_layout: D3D12_PLACED_SUBRESOURCE_FOOTPRINT,
    upload_size: u64,
    texture_is_copy_source: bool,
    width: u32,
    height: u32,
    window: NativeVideoWindow,
}

impl D3d12Host {
    /// Create the native window, D3D12 device/queue and DXGI flip-model swap chain.
    ///
    /// The initial window size and swap-chain image size are identical.
    pub fn initialize(width: u32, height: u32) -> Result<Self> {
        Self::initialize_with_window_size(width, height, width, height)
    }

    /// Initialize a logical render surface and a separately sized native window.
    ///
    /// DXGI's stretch scaling presents the logical frame across the window
    /// client area. This is useful for SMS native-resolution frames without
    /// requiring the emulation core to render at the desktop window size.
    pub fn initialize_with_window_size(
        render_width: u32,
        render_height: u32,
        window_width: u32,
        window_height: u32,
    ) -> Result<Self> {
        if render_width == 0 || render_height == 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "render dimensions must be non-zero",
            ));
        }
        let window = NativeVideoWindow::create(window_width, window_height)?;
        Self::initialize_with_window(render_width, render_height, window)
    }

    /// Initialize a swap chain for an HWND borrowed from a GUI toolkit.
    ///
    /// The HWND must be validated on its owning GUI thread before this call
    /// and remain alive until the presentation session has shut down.
    pub(crate) fn initialize_for_existing_window(
        render_width: u32,
        render_height: u32,
        window_handle: usize,
    ) -> Result<Self> {
        if render_width == 0 || render_height == 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "render dimensions must be non-zero",
            ));
        }
        let window = NativeVideoWindow::from_existing_handle(window_handle);
        Self::initialize_with_window(render_width, render_height, window)
    }

    fn initialize_with_window(
        render_width: u32,
        render_height: u32,
        window: NativeVideoWindow,
    ) -> Result<Self> {
        let factory: IDXGIFactory2 =
            match unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default()) } {
                Ok(factory) => factory,
                Err(error) => {
                    sega_observability::tracing::error!(
                        target: "sega::video::d3d12",
                        error = %error,
                        "DXGI factory creation failed"
                    );
                    return Err(error);
                }
            };

        let mut device: Option<ID3D12Device> = None;
        if let Err(error) = unsafe { D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device) }
        {
            sega_observability::tracing::error!(
                target: "sega::video::d3d12",
                feature_level = "11_0",
                error = %error,
                "D3D12 device creation failed"
            );
            return Err(error);
        }
        let device: ID3D12Device = device.ok_or_else(Error::from_thread)?;
        let adapter_luid = unsafe { device.GetAdapterLuid() };
        sega_observability::tracing::info!(
            target: "sega::video::d3d12",
            adapter_luid_low = adapter_luid.LowPart,
            adapter_luid_high = adapter_luid.HighPart,
            feature_level = "11_0",
            render_width,
            render_height,
            "D3D12 device initialized on default adapter"
        );

        let queue_desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let queue: ID3D12CommandQueue = match unsafe { device.CreateCommandQueue(&queue_desc) } {
            Ok(queue) => queue,
            Err(error) => {
                sega_observability::tracing::error!(
                    target: "sega::video::d3d12",
                    error = %error,
                    "D3D12 command queue creation failed"
                );
                return Err(error);
            }
        };

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: render_width,
            Height: render_height,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: SWAP_CHAIN_BUFFER_COUNT,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
            ..Default::default()
        };

        let swap_chain1 = match unsafe {
            factory.CreateSwapChainForHwnd(&queue, window.hwnd(), &swap_chain_desc, None, None)
        } {
            Ok(swap_chain) => swap_chain,
            Err(error) => {
                sega_observability::tracing::error!(
                    target: "sega::video::d3d12",
                    render_width,
                    render_height,
                    format = "R8G8B8A8_UNORM",
                    buffer_count = SWAP_CHAIN_BUFFER_COUNT,
                    error = %error,
                    "D3D12 swap chain creation failed"
                );
                return Err(error);
            }
        };
        sega_observability::tracing::info!(
            target: "sega::video::d3d12",
            render_width,
            render_height,
            format = "R8G8B8A8_UNORM",
            buffer_count = SWAP_CHAIN_BUFFER_COUNT,
            maximum_frame_latency = 1,
            "D3D12 swap chain created"
        );
        unsafe {
            factory.MakeWindowAssociation(
                window.hwnd(),
                windows::Win32::Graphics::Dxgi::DXGI_MWA_NO_ALT_ENTER,
            )?;
        }

        let swap_chain2: IDXGISwapChain2 = swap_chain1.cast()?;
        unsafe { swap_chain2.SetMaximumFrameLatency(1)? };
        let frame_latency_handle = unsafe { swap_chain2.GetFrameLatencyWaitableObject() };
        if frame_latency_handle.is_invalid() {
            return Err(Error::from_thread());
        }
        let swap_chain: IDXGISwapChain3 = swap_chain2.cast()?;

        let rtv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: SWAP_CHAIN_BUFFER_COUNT,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };
        let rtv_heap: ID3D12DescriptorHeap =
            unsafe { device.CreateDescriptorHeap(&rtv_heap_desc)? };
        let descriptor_increment =
            unsafe { device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) };
        let first_rtv = unsafe { rtv_heap.GetCPUDescriptorHandleForHeapStart() };

        let mut render_targets = Vec::with_capacity(SWAP_CHAIN_BUFFER_COUNT as usize);
        let mut rtv_handles = Vec::with_capacity(SWAP_CHAIN_BUFFER_COUNT as usize);
        for index in 0..SWAP_CHAIN_BUFFER_COUNT {
            let resource: ID3D12Resource = unsafe { swap_chain.GetBuffer(index)? };
            let handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: first_rtv.ptr + index as usize * descriptor_increment as usize,
            };
            unsafe { device.CreateRenderTargetView(&resource, None, handle) };
            render_targets.push(resource);
            rtv_handles.push(handle);
        }

        let command_allocator: ID3D12CommandAllocator =
            unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)? };
        let command_list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &command_allocator,
                None::<&ID3D12PipelineState>,
            )?
        };
        unsafe { command_list.Close()? };

        let texture_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: u64::from(render_width),
            Height: render_height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };
        let mut upload_layout = D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default();
        let mut upload_rows = 0_u32;
        let mut upload_row_size = 0_u64;
        let mut upload_size = 0_u64;
        unsafe {
            device.GetCopyableFootprints(
                &texture_desc,
                0,
                1,
                0,
                Some(&mut upload_layout),
                Some(&mut upload_rows),
                Some(&mut upload_row_size),
                Some(&mut upload_size),
            );
        }
        if upload_rows != render_height || upload_row_size != u64::from(render_width) * 4 {
            return Err(Error::from_thread());
        }

        let default_heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        };
        let mut frame_texture = None;
        unsafe {
            device.CreateCommittedResource(
                &default_heap,
                D3D12_HEAP_FLAG_NONE,
                &texture_desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut frame_texture,
            )?;
        }
        let frame_texture: ID3D12Resource = frame_texture.ok_or_else(Error::from_thread)?;

        let upload_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: upload_size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: D3D12_RESOURCE_FLAG_NONE,
        };
        let upload_heap = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_UPLOAD,
            ..default_heap
        };
        let mut upload_buffer = None;
        unsafe {
            device.CreateCommittedResource(
                &upload_heap,
                D3D12_HEAP_FLAG_NONE,
                &upload_desc,
                D3D12_RESOURCE_STATE_GENERIC_READ,
                None,
                &mut upload_buffer,
            )?;
        }
        let upload_buffer: ID3D12Resource = upload_buffer.ok_or_else(Error::from_thread)?;

        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE)? };
        let fence_event = unsafe { CreateEventW(None, false, false, None)? };

        Ok(Self {
            _factory: factory,
            _device: device,
            queue,
            swap_chain,
            swap_chain2,
            frame_latency_handle,
            rtv_heap,
            render_targets,
            rtv_handles,
            command_allocator,
            command_list,
            fence,
            fence_event,
            last_submitted_fence: 0,
            frame_texture,
            upload_buffer,
            upload_layout,
            upload_size,
            texture_is_copy_source: false,
            width: render_width,
            height: render_height,
            window,
        })
    }

    /// Pump the Rust-owned native window message queue.
    pub fn pump_messages(&self) -> bool {
        self.window.pump_messages()
    }

    /// Return the current DXGI back-buffer index.
    pub fn current_back_buffer_index(&self) -> u32 {
        unsafe { self.swap_chain.GetCurrentBackBufferIndex() }
    }

    /// Return the configured presentation dimensions.
    pub const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Return the configured maximum queued presentation latency.
    pub fn maximum_frame_latency(&self) -> Result<u32> {
        unsafe { self.swap_chain2.GetMaximumFrameLatency() }
    }

    /// Return whether the native window is minimized.
    ///
    /// The logical swap-chain dimensions remain fixed; DXGI stretch scaling
    /// adapts presentation to the client area without changing emulated pixels.
    pub fn is_minimized(&self) -> bool {
        self.window.is_minimized()
    }

    /// Block the presentation thread until DXGI allows another frame.
    ///
    /// Call this only from the host/render thread. It must never gate machine
    /// stepping or Scheduler time.
    pub fn wait_for_frame_slot(&self) -> Result<()> {
        loop {
            let result = unsafe {
                WaitForSingleObjectEx(self.frame_latency_handle, PRESENTATION_WAIT_POLL_MS, false)
            };

            if result == WAIT_OBJECT_0 {
                return Ok(());
            }
            if result == WAIT_TIMEOUT {
                if !self.window.pump_messages() {
                    return Err(Error::new(
                        windows::core::HRESULT(0x8000_4004_u32 as i32),
                        "native presentation window closed while waiting for a frame slot",
                    ));
                }

                // The frame-latency handle is not guaranteed to signal after
                // adapter removal. Poll the D3D12 device reason on each finite
                // wait so the worker can fail fast instead of blocking forever.
                self.device_removed_reason()?;
                continue;
            }

            return Err(Error::from_thread());
        }
    }

    /// Present a synthetic color-bar pattern through the real D3D12 swap chain.
    ///
    /// This is a renderer bring-up diagnostic, not an emulator framebuffer.
    /// The command allocator is not reset until the previous submission's
    /// fence has completed. The back buffer is transitioned to RENDER_TARGET
    /// before clear and back to PRESENT before DXGI Present.
    pub fn present_test_pattern(&mut self) -> Result<()> {
        self.wait_for_frame_slot()?;
        self.wait_for_gpu()?;

        let buffer_index = self.current_back_buffer_index() as usize;
        let back_buffer = &self.render_targets[buffer_index];
        let rtv = self.rtv_handles[buffer_index];

        unsafe {
            self.command_allocator.Reset()?;
            self.command_list
                .Reset(&self.command_allocator, None::<&ID3D12PipelineState>)?;

            let to_render_target = transition_barrier(
                back_buffer,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );
            self.command_list.ResourceBarrier(&[to_render_target]);
            self.command_list
                .OMSetRenderTargets(1, Some(&rtv), false, None);

            self.command_list
                .ClearRenderTargetView(rtv, &[0.025, 0.035, 0.055, 1.0], None);

            let band_top = (self.height / 8) as i32;
            let band_bottom = (self.height * 7 / 8) as i32;
            let colors = [
                [0.85, 0.08, 0.08, 1.0],
                [0.08, 0.78, 0.12, 1.0],
                [0.08, 0.20, 0.90, 1.0],
                [0.85, 0.80, 0.08, 1.0],
                [0.80, 0.08, 0.82, 1.0],
                [0.08, 0.78, 0.82, 1.0],
            ];
            for (bar, color) in colors.iter().enumerate() {
                let left = (self.width as usize * bar / colors.len()) as i32;
                let right = (self.width as usize * (bar + 1) / colors.len()) as i32;
                let rect = RECT {
                    left,
                    top: band_top,
                    right,
                    bottom: band_bottom,
                };
                self.command_list.ClearRenderTargetView(
                    rtv,
                    color,
                    Some(std::slice::from_ref(&rect)),
                );
            }

            let to_present = transition_barrier(
                back_buffer,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            self.command_list.ResourceBarrier(&[to_present]);
            self.command_list.Close()?;

            let command_list: ID3D12CommandList = self.command_list.cast()?;
            self.queue.ExecuteCommandLists(&[Some(command_list)]);

            // Queue the fence before Present so a DXGI presentation failure
            // cannot leave submitted GPU work without a retirement signal.
            let next_fence = self.last_submitted_fence.checked_add(1).ok_or_else(|| {
                Error::new(
                    windows::core::HRESULT(0x8000_4005_u32 as i32),
                    "D3D12 fence value exhausted",
                )
            })?;
            self.queue.Signal(&self.fence, next_fence)?;
            self.last_submitted_fence = next_fence;
            let present_result = self.swap_chain.Present(1, DXGI_PRESENT(0)).ok();
            if let Err(error) = &present_result {
                sega_observability::tracing::error!(
                    target: "sega::video::d3d12",
                    error = %error,
                    device_removed_reason = ?self.device_removed_reason().err(),
                    "D3D12 Present failed"
                );
            }
            present_result?;
        }

        Ok(())
    }

    /// Consume and present the newest pending frame from a Rust mailbox.
    ///
    /// Call this from the dedicated render thread after pumping the HWND
    /// message queue. The mailbox lock is released before any GPU wait, upload
    /// or Present call. It must not be called by the emulation/Scheduler thread.
    ///
    /// Returns the presented frame sequence, or None when no frame is pending.
    pub fn present_latest_from_mailbox(
        &mut self,
        mailbox: &FrameMailbox,
    ) -> core::result::Result<Option<u64>, MailboxPresentError> {
        let Some(frame) = mailbox
            .take_latest()
            .map_err(MailboxPresentError::Mailbox)?
        else {
            return Ok(None);
        };

        let sequence = frame.sequence();
        self.present_frame(&frame)
            .map_err(MailboxPresentError::Graphics)?;
        Ok(Some(sequence))
    }

    /// Upload and present one immutable emulator frame.
    ///
    /// The host's swap-chain dimensions must match the frame dimensions.
    /// RGB565 and RGB888 are expanded to RGBA8888 in the mapped upload buffer;
    /// RGBA8888 rows are copied directly. D3D12's GetCopyableFootprints supplies the
    /// required row pitch, so the source frame stride is never assumed to be
    /// GPU-aligned.
    pub fn present_frame(&mut self, frame: &VideoFrame) -> Result<()> {
        if frame.width() != self.width || frame.height() != self.height {
            sega_observability::tracing::error!(
                target: "sega::video::d3d12",
                frame_width = frame.width(),
                frame_height = frame.height(),
                swap_chain_width = self.width,
                swap_chain_height = self.height,
                "video frame dimensions do not match D3D12 swap chain"
            );
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "VideoFrame dimensions must match the initialized DXGI swap chain",
            ));
        }

        self.wait_for_frame_slot()?;
        self.wait_for_gpu()?;
        self.write_frame_to_upload(frame)?;

        let buffer_index = self.current_back_buffer_index() as usize;
        let back_buffer = &self.render_targets[buffer_index];

        unsafe {
            self.command_allocator.Reset()?;
            self.command_list
                .Reset(&self.command_allocator, None::<&ID3D12PipelineState>)?;

            if self.texture_is_copy_source {
                let to_copy_dest = transition_barrier(
                    &self.frame_texture,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                );
                self.command_list.ResourceBarrier(&[to_copy_dest]);
            }

            let upload_location = D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(self.upload_buffer.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: self.upload_layout,
                },
            };
            let texture_location = D3D12_TEXTURE_COPY_LOCATION {
                pResource: ManuallyDrop::new(Some(self.frame_texture.clone())),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };
            self.command_list
                .CopyTextureRegion(&texture_location, 0, 0, 0, &upload_location, None);

            let texture_to_source = transition_barrier(
                &self.frame_texture,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            );
            let backbuffer_to_dest = transition_barrier(
                back_buffer,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_COPY_DEST,
            );
            self.command_list
                .ResourceBarrier(&[texture_to_source, backbuffer_to_dest]);
            self.command_list
                .CopyResource(back_buffer, &self.frame_texture);

            let backbuffer_to_present = transition_barrier(
                back_buffer,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            self.command_list.ResourceBarrier(&[backbuffer_to_present]);
            self.command_list.Close()?;

            let command_list: ID3D12CommandList = self.command_list.cast()?;
            self.queue.ExecuteCommandLists(&[Some(command_list)]);
            self.texture_is_copy_source = true;

            let next_fence = self.last_submitted_fence.checked_add(1).ok_or_else(|| {
                Error::new(
                    windows::core::HRESULT(0x8000_4005_u32 as i32),
                    "D3D12 fence value exhausted",
                )
            })?;
            self.queue.Signal(&self.fence, next_fence)?;
            self.last_submitted_fence = next_fence;
            let present_result = self.swap_chain.Present(1, DXGI_PRESENT(0)).ok();
            if let Err(error) = &present_result {
                sega_observability::tracing::error!(
                    target: "sega::video::d3d12",
                    frame_sequence = frame.sequence(),
                    error = %error,
                    device_removed_reason = ?self.device_removed_reason().err(),
                    "D3D12 mailbox frame presentation failed"
                );
            }
            present_result?;
        }

        Ok(())
    }

    fn write_frame_to_upload(&self, frame: &VideoFrame) -> Result<()> {
        let row_bytes = (frame.width() as usize)
            .checked_mul(4)
            .ok_or_else(Error::from_thread)?;
        let row_pitch = self.upload_layout.Footprint.RowPitch as usize;
        let required_size = self
            .upload_layout
            .Offset
            .checked_add(
                (row_pitch as u64)
                    .checked_mul(u64::from(frame.height()))
                    .ok_or_else(Error::from_thread)?,
            )
            .ok_or_else(Error::from_thread)?;
        if row_pitch < row_bytes || required_size > self.upload_size {
            return Err(Error::from_thread());
        }
        if (0..frame.height()).any(|row| frame.row(row).is_none()) {
            return Err(Error::from_thread());
        }

        let read_range = D3D12_RANGE { Begin: 0, End: 0 };
        let mut mapped = null_mut();
        unsafe {
            self.upload_buffer
                .Map(0, Some(&read_range), Some(&mut mapped))?;
        }
        if mapped.is_null() {
            unsafe { self.upload_buffer.Unmap(0, None) };
            return Err(Error::from_thread());
        }

        let destination = unsafe { mapped.cast::<u8>().add(self.upload_layout.Offset as usize) };

        for y in 0..frame.height() {
            let destination_row = unsafe {
                slice::from_raw_parts_mut(destination.add(y as usize * row_pitch), row_bytes)
            };
            if !convert_frame_row_to_rgba8888(frame, y, destination_row) {
                unsafe { self.upload_buffer.Unmap(0, None) };
                return Err(Error::from_thread());
            }
        }

        let written_range = D3D12_RANGE {
            Begin: self.upload_layout.Offset as usize,
            End: self.upload_size as usize,
        };
        unsafe {
            self.upload_buffer.Unmap(0, Some(&written_range));
        }
        Ok(())
    }

    fn wait_for_gpu(&self) -> Result<()> {
        let completed_value = unsafe { self.fence.GetCompletedValue() };
        // D3D12 returns UINT64_MAX when the device has been removed. Preserve
        // the driver's actual removal reason instead of flattening it to a
        // generic timeout or successful fence completion.
        if completed_value == u64::MAX {
            return match self.device_removed_reason() {
                Err(reason) => Err(reason),
                Ok(()) => Err(Error::new(
                    windows::core::HRESULT(0x887A_0005_u32 as i32),
                    "D3D12 fence returned the device-removed sentinel",
                )),
            };
        }
        if self.last_submitted_fence == 0 || completed_value >= self.last_submitted_fence {
            return self.device_removed_reason();
        }

        unsafe {
            self.fence
                .SetEventOnCompletion(self.last_submitted_fence, self.fence_event)?;
        }

        loop {
            let result = unsafe {
                WaitForSingleObjectEx(self.fence_event, PRESENTATION_WAIT_POLL_MS, false)
            };

            if result == WAIT_OBJECT_0 {
                return self.device_removed_reason();
            }
            if result == WAIT_TIMEOUT {
                if !self.window.pump_messages() {
                    return Err(Error::new(
                        windows::core::HRESULT(0x8000_4004_u32 as i32),
                        "native presentation window closed while waiting for the GPU",
                    ));
                }
                self.device_removed_reason()?;
                continue;
            }

            return Err(Error::from_thread());
        }
    }

    /// Query the original D3D12 device-removal reason.
    ///
    /// Returns Ok only while the device remains valid. When the adapter is
    /// removed/reset or the driver reports another device-loss condition, the
    /// returned Windows error preserves the original HRESULT for the session.
    pub fn device_removed_reason(&self) -> Result<()> {
        unsafe { self._device.GetDeviceRemovedReason() }
    }
}

/// Convert one immutable source row to tightly packed RGBA8888 bytes.
///
/// Returns false when the source row or destination length is invalid. This
/// pure conversion is kept separate from mapped GPU memory so its pixel math
/// can be tested without creating a D3D12 device.
fn convert_frame_row_to_rgba8888(frame: &VideoFrame, y: u32, destination: &mut [u8]) -> bool {
    let Some(source) = frame.row(y) else {
        return false;
    };
    let Some(expected_len) = (frame.width() as usize).checked_mul(4) else {
        return false;
    };
    if destination.len() != expected_len {
        return false;
    }

    match frame.pixel_format() {
        PixelFormat::Rgba8888 => {
            if source.len() != expected_len {
                return false;
            }
            destination.copy_from_slice(source);
        }
        PixelFormat::Rgb888 => {
            let Some(expected_source_len) = (frame.width() as usize).checked_mul(3) else {
                return false;
            };
            if source.len() != expected_source_len {
                return false;
            }

            let (source_pixels, []) = source.as_chunks::<3>() else {
                return false;
            };
            let (destination_pixels, []) = destination.as_chunks_mut::<4>() else {
                return false;
            };

            for (source_pixel, destination_pixel) in
                source_pixels.iter().zip(destination_pixels.iter_mut())
            {
                destination_pixel[0] = source_pixel[0];
                destination_pixel[1] = source_pixel[1];
                destination_pixel[2] = source_pixel[2];
                destination_pixel[3] = u8::MAX;
            }
        }
        PixelFormat::Rgb565Le => {
            let Some(expected_source_len) = (frame.width() as usize).checked_mul(2) else {
                return false;
            };
            if source.len() != expected_source_len {
                return false;
            }

            let (source_pixels, []) = source.as_chunks::<2>() else {
                return false;
            };
            let (destination_pixels, []) = destination.as_chunks_mut::<4>() else {
                return false;
            };

            for (source_pixel, destination_pixel) in
                source_pixels.iter().zip(destination_pixels.iter_mut())
            {
                let pixel = u16::from_le_bytes(*source_pixel);
                let red = ((pixel >> 11) & 0x1F) as u8;
                let green = ((pixel >> 5) & 0x3F) as u8;
                let blue = (pixel & 0x1F) as u8;
                destination_pixel[0] = (red << 3) | (red >> 2);
                destination_pixel[1] = (green << 2) | (green >> 4);
                destination_pixel[2] = (blue << 3) | (blue >> 2);
                destination_pixel[3] = u8::MAX;
            }
        }
    }
    true
}

impl Drop for D3d12Host {
    fn drop(&mut self) {
        let _ = self.wait_for_gpu();

        if !self.frame_latency_handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.frame_latency_handle);
            }
            self.frame_latency_handle = HANDLE::default();
        }
        if !self.fence_event.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.fence_event);
            }
            self.fence_event = HANDLE::default();
        }

        // The HWND is the final field, so its owner is dropped after DXGI.
        let _ = &self.rtv_heap;
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::{
        D3d12ChildWindow, NativeVideoWindow, PixelFormat, VideoFrame, convert_frame_row_to_rgba8888,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, IsWindow, SW_MINIMIZE, ShowWindow, WS_CLIPCHILDREN,
    };

    #[test]
    fn child_viewport_is_created_and_resized_on_parent_gui_thread() {
        let parent = NativeVideoWindow::create(640, 480).unwrap();
        let child = D3d12ChildWindow::create(parent.hwnd().0 as usize).unwrap();

        assert!(unsafe { IsWindow(Some(child.hwnd)).as_bool() });
        child.set_bounds(8, 12, 256, 192).unwrap();
    }

    #[test]
    fn child_viewport_enables_and_restores_parent_clip_children_style() {
        let parent = NativeVideoWindow::create(640, 480).unwrap();
        let parent_hwnd = parent.hwnd();
        let original_style = unsafe { GetWindowLongPtrW(parent_hwnd, GWL_STYLE) };
        let originally_clipped = original_style & (WS_CLIPCHILDREN.0 as isize) != 0;

        {
            let child = D3d12ChildWindow::create(parent_hwnd.0 as usize).unwrap();
            let child_style = unsafe { GetWindowLongPtrW(parent_hwnd, GWL_STYLE) };
            assert_ne!(child_style & (WS_CLIPCHILDREN.0 as isize), 0);
            drop(child);
        }

        let restored_style = unsafe { GetWindowLongPtrW(parent_hwnd, GWL_STYLE) };
        assert_eq!(
            restored_style & (WS_CLIPCHILDREN.0 as isize) != 0,
            originally_clipped
        );
    }

    #[test]
    fn external_hwnd_rejects_calls_from_non_owner_thread() {
        let parent = NativeVideoWindow::create(320, 240).unwrap();
        let handle = parent.hwnd().0 as usize;

        let validation = thread::spawn(move || NativeVideoWindow::validate_existing_handle(handle))
            .join()
            .unwrap();

        assert!(validation.is_err());
    }

    #[test]
    fn borrowed_child_detects_minimized_root_window() {
        let parent = NativeVideoWindow::create(320, 240).unwrap();
        let child = D3d12ChildWindow::create(parent.hwnd().0 as usize).unwrap();
        let borrowed = NativeVideoWindow::from_existing_handle(child.handle());

        assert!(!borrowed.is_minimized());
        unsafe {
            let _ = ShowWindow(parent.hwnd(), SW_MINIMIZE);
        }
        assert!(borrowed.is_minimized());
    }

    #[test]
    fn converts_rgb565_primary_colors_to_rgba8888() {
        let frame = VideoFrame::new(
            1,
            3,
            1,
            PixelFormat::Rgb565Le,
            6,
            [
                0x00, 0xF8, // red
                0xE0, 0x07, // green
                0x1F, 0x00, // blue
            ]
            .into(),
        )
        .unwrap();
        let mut output = [0; 12];

        assert!(convert_frame_row_to_rgba8888(&frame, 0, &mut output));
        assert_eq!(output, [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn converts_rgb565_active_row_without_padding() {
        let frame = VideoFrame::new(
            1,
            1,
            1,
            PixelFormat::Rgb565Le,
            4,
            vec![0x00, 0xF8, 0xAA, 0xBB],
        )
        .unwrap();
        let mut output = [0; 4];

        assert!(convert_frame_row_to_rgba8888(&frame, 0, &mut output));
        assert_eq!(output, [255, 0, 0, 255]);
    }

    #[test]
    fn converts_rgb888_primary_colors_to_rgba8888() {
        let frame = VideoFrame::new(
            2,
            3,
            1,
            PixelFormat::Rgb888,
            9,
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
        )
        .unwrap();
        let mut output = [0; 12];

        assert!(convert_frame_row_to_rgba8888(&frame, 0, &mut output));
        assert_eq!(output, [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255]);
    }

    #[test]
    fn converts_rgb888_active_row_without_padding() {
        let frame =
            VideoFrame::new(3, 1, 1, PixelFormat::Rgb888, 5, vec![12, 34, 56, 90, 91]).unwrap();
        let mut output = [0; 4];

        assert!(convert_frame_row_to_rgba8888(&frame, 0, &mut output));
        assert_eq!(output, [12, 34, 56, 255]);
    }

    #[test]
    fn copies_rgba8888_active_row_without_padding() {
        let frame = VideoFrame::new(
            2,
            1,
            1,
            PixelFormat::Rgba8888,
            8,
            vec![10, 20, 30, 40, 90, 91, 92, 93],
        )
        .unwrap();
        let mut output = [0; 4];

        assert!(convert_frame_row_to_rgba8888(&frame, 0, &mut output));
        assert_eq!(output, [10, 20, 30, 40]);
    }

    #[test]
    fn rejects_invalid_row_or_destination_size() {
        let frame = VideoFrame::new(3, 1, 1, PixelFormat::Rgb565Le, 2, vec![0, 0]).unwrap();

        assert!(!convert_frame_row_to_rgba8888(&frame, 1, &mut [0; 4]));
        assert!(!convert_frame_row_to_rgba8888(&frame, 0, &mut [0; 3]));
    }
}
