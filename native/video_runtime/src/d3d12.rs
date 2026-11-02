//! Initial native Direct3D 12 host bootstrap for Windows x64.
//!
//! This milestone owns the Win32 presentation window, DXGI factory/swap chain,
//! D3D12 device and direct command queue. It intentionally does not submit or
//! present frames yet: the upload/copy pipeline, resource barriers and fence
//! retirement are implemented in the next milestone.

use windows::Win32::Foundation::{CloseHandle, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_COMMAND_QUEUE_FLAG_NONE,
    D3D12_COMMAND_QUEUE_PRIORITY_NORMAL, D3D12CreateDevice, ID3D12CommandQueue, ID3D12Device,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIFactory2, IDXGISwapChain2, IDXGISwapChain3,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{INFINITE, WaitForSingleObjectEx};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, PostQuitMessage, RegisterClassW, SW_SHOW,
    ShowWindow, TranslateMessage, WNDCLASSW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_DESTROY, WS_OVERLAPPEDWINDOW,
};
use windows::core::{Error, Interface, Result, w};

/// Native Win32 window owned by the Rust video host.
pub struct NativeVideoWindow {
    hwnd: HWND,
}

impl NativeVideoWindow {
    /// Create and show the native presentation window.
    pub fn create(width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "window dimensions must be non-zero",
            ));
        }

        let instance = unsafe { GetModuleHandleW(None)? };
        let class_name = w!("SegaEmulatorD3D12Window");
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: HINSTANCE(instance.0),
            lpszClassName: class_name,
            ..Default::default()
        };

        let atom = unsafe { RegisterClassW(&class) };
        if atom == 0 {
            return Err(Error::from_thread());
        }

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

        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            // ShowWindow triggers the initial non-client paint/message sequence.
        }

        Ok(Self { hwnd })
    }

    /// Return the native HWND without transferring its ownership.
    pub const fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Pump pending Win32 messages; return false after WM_QUIT.
    pub fn pump_messages(&self) -> bool {
        let mut message = MSG::default();
        let mut running = true;

        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                if message.message == windows::Win32::UI::WindowsAndMessaging::WM_QUIT {
                    running = false;
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        running
    }
}

impl Drop for NativeVideoWindow {
    fn drop(&mut self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            self.hwnd = HWND::default();
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return 0;
    }

    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Direct3D 12/DXGI initialization state.
///
/// The swap chain uses the DXGI flip-discard model with a frame-latency
/// waitable object and a maximum queued latency of one frame. This object does
/// not yet expose frame submission because no command-list/upload pipeline has
/// been installed in this bootstrap milestone.
pub struct D3d12Host {
    window: NativeVideoWindow,
    _factory: IDXGIFactory2,
    _device: ID3D12Device,
    _queue: ID3D12CommandQueue,
    swap_chain: IDXGISwapChain3,
    swap_chain2: IDXGISwapChain2,
    frame_latency_handle: HANDLE,
    width: u32,
    height: u32,
}

impl D3d12Host {
    /// Create the native window, D3D12 device/queue and DXGI flip-model swap chain.
    pub fn initialize(width: u32, height: u32) -> Result<Self> {
        let window = NativeVideoWindow::create(width, height)?;
        let factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default())? };

        let mut device = None;
        unsafe {
            D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device)?;
        }
        let device = device.ok_or_else(Error::from_thread)?;

        let queue_desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: D3D12_COMMAND_QUEUE_PRIORITY_NORMAL.0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let queue: ID3D12CommandQueue = unsafe { device.CreateCommandQueue(&queue_desc)? };

        let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
            ..Default::default()
        };

        let swap_chain1 = unsafe {
            factory.CreateSwapChainForHwnd(&queue, window.hwnd(), &swap_chain_desc, None, None)?
        };
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

        Ok(Self {
            window,
            _factory: factory,
            _device: device,
            _queue: queue,
            swap_chain,
            swap_chain2,
            frame_latency_handle,
            width,
            height,
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

    /// Block the presentation thread until DXGI allows another frame.
    ///
    /// Call this only from the host/render thread. It must never gate machine
    /// stepping or Scheduler time. The swap chain was created with the
    /// frame-latency waitable-object flag and a maximum latency of one.
    pub fn wait_for_frame_slot(&self) -> Result<()> {
        let result = unsafe { WaitForSingleObjectEx(self.frame_latency_handle, INFINITE, false) };
        if result.0 == 0 {
            Ok(())
        } else {
            Err(Error::from_thread())
        }
    }

    /// Return whether the frame-latency waitable object was created.
    pub fn has_frame_latency_waitable_object(&self) -> bool {
        !self.frame_latency_handle.is_invalid()
    }
}

impl Drop for D3d12Host {
    fn drop(&mut self) {
        if !self.frame_latency_handle.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.frame_latency_handle);
            }
            self.frame_latency_handle = HANDLE::default();
        }
    }
}
