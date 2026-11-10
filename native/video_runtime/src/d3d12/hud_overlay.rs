//! Native, parent-clipped telemetry HUD layered above the D3D12 child viewport.
//!
//! The HUD is a WS_CHILD sibling of the D3D12 presentation HWND. It is never a
//! top-level or topmost window, and its layered transparent surface passes
//! pointer input through to the child window underneath.

use core::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::{
    ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLORREF, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    HGDIOBJ, PAINTSTRUCT, SetBkMode, SetTextColor, TRANSPARENT, DT_LEFT, DT_NOPREFIX, DT_TOP,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect,
    GetWindowLongPtrW, GetWindowTextW, RegisterClassW, SetLayeredWindowAttributes, SetWindowPos,
    SetWindowTextW, ShowWindow, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_SHOWWINDOW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_CHILD,
    WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT, WS_VISIBLE, GWL_EXSTYLE,
    GWL_STYLE, HWND_TOP, HTTRANSPARENT, LWA_COLORKEY, MA_NOACTIVATE,
};
use windows::core::{Error, PCWSTR, Result, w};

/// A thread-affine transparent child HWND for the native SMS telemetry HUD.
///
/// The parent is the eframe root HWND. The HUD is a sibling of the D3D12 video
/// child and is explicitly placed at the top of the parent's child z-order.
pub struct D3d12HudOverlay {
    hwnd: HWND,
    parent_hwnd: HWND,
    _thread_affinity: PhantomData<Rc<()>>,
}

impl D3d12HudOverlay {
    /// Create the transparent HUD child under an existing GUI-owned parent HWND.
    pub fn create(parent_handle: usize) -> Result<Self> {
        NativeVideoWindowValidation::validate(parent_handle)?;
        let instance = unsafe { GetModuleHandleW(None)? };
        register_hud_window_class(HINSTANCE(instance.0))?;

        let parent = HWND(parent_handle as *mut c_void);
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(
                    WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0,
                ),
                w!("SegaEmulatorTelemetryHud"),
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

        if let Err(error) = unsafe {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY)
        } {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(error);
        }

        let overlay = Self {
            hwnd,
            parent_hwnd: parent,
            _thread_affinity: PhantomData,
        };
        overlay.set_bounds(0, 0, 1, 1)?;
        Ok(overlay)
    }

    /// Return the child HWND without transferring ownership.
    pub fn handle(&self) -> usize {
        self.hwnd.0 as usize
    }

    /// Return the parent HWND without transferring ownership.
    pub fn parent_handle(&self) -> usize {
        self.parent_hwnd.0 as usize
    }

    /// Replace the displayed text and invalidate the layered child surface.
    pub fn set_text(&self, text: &str) -> Result<()> {
        let safe_text = text.replace('\0', " ").replace('\n', "\r\n");
        let wide = safe_text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<_>>();
        unsafe {
            SetWindowTextW(self.hwnd, PCWSTR(wide.as_ptr()))?;
        }
        Ok(())
    }

    /// Move and resize the HUD in parent-client physical pixels.
    ///
    /// HWND_TOP changes only the sibling z-order under the supplied parent; it
    /// does not promote the child into the desktop topmost window band.
    pub fn set_bounds(&self, x: i32, y: i32, width: i32, height: i32) -> Result<()> {
        if x < 0 || y < 0 || width <= 0 || height <= 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "HUD bounds require a non-negative origin and positive dimensions",
            ));
        }

        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOP),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )?;
        }
        Ok(())
    }

    /// Show or hide the child HUD without changing focus or desktop z-order.
    pub fn set_visible(&self, visible: bool) {
        unsafe {
            ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    /// Report whether the HWND has the required child/layered/pass-through styles.
    pub fn composition_contract(&self) -> (bool, bool, bool, bool) {
        let style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_STYLE) };
        let ex_style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        (
            style & (WS_CHILD.0 as isize) != 0,
            ex_style & (WS_EX_LAYERED.0 as isize) != 0,
            ex_style & (WS_EX_TRANSPARENT.0 as isize) != 0,
            ex_style & (windows::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST.0 as isize) == 0,
        )
    }
}

impl Drop for D3d12HudOverlay {
    fn drop(&mut self) {
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
            self.hwnd = HWND::default();
        }
        self.parent_hwnd = HWND::default();
    }
}

fn register_hud_window_class(instance: HINSTANCE) -> Result<()> {
    let class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(hud_window_proc),
        hInstance: instance,
        lpszClassName: w!("SegaEmulatorTelemetryHud"),
        ..Default::default()
    };

    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 && unsafe { GetLastError() } != ERROR_CLASS_ALREADY_EXISTS {
        return Err(Error::from_thread());
    }
    Ok(())
}

unsafe extern "system" fn hud_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => LRESULT(1),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut paint) };
            if !hdc.0.is_null() {
                let mut rect = RECT::default();
                if unsafe { GetClientRect(hwnd, &mut rect) }.is_ok() {
                    let black = unsafe { CreateSolidBrush(COLORREF(0)) };
                    unsafe {
                        FillRect(hdc, &rect, black);
                        let _ = DeleteObject(HGDIOBJ(black.0));
                        SetBkMode(hdc, TRANSPARENT);
                        SetTextColor(hdc, COLORREF(0x00AA_FFAA));
                    }

                    let mut text = [0_u16; 512];
                    let length = unsafe { GetWindowTextW(hwnd, &mut text) };
                    if length > 0 {
                        rect.left += 7;
                        rect.top += 5;
                        rect.right -= 5;
                        rect.bottom -= 5;
                        unsafe {
                            DrawTextW(
                                hdc,
                                &mut text[..usize::try_from(length).unwrap_or(0)],
                                &mut rect,
                                DT_LEFT | DT_TOP | DT_NOPREFIX,
                            );
                        }
                    }
                }
            }
            unsafe {
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

struct NativeVideoWindowValidation;

impl NativeVideoWindowValidation {
    fn validate(parent_handle: usize) -> Result<()> {
        if parent_handle == 0 {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "HUD parent HWND must be non-zero",
            ));
        }
        let parent = HWND(parent_handle as *mut c_void);
        if unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(parent)) }.as_bool() {
            Ok(())
        } else {
            Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "HUD parent HWND is not a valid window",
            ))
        }
    }
}
