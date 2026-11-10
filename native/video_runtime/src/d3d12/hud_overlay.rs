//! Native, parent-clipped telemetry HUD layered above the D3D12 child viewport.
//!
//! The HUD is a WS_CHILD sibling of the D3D12 presentation HWND. It is never a
//! top-level or topmost window, and its layered transparent surface passes
//! pointer input through to the child window underneath.

use core::ffi::c_void;
use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, RECT,
    WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BeginPaint, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush,
    DEFAULT_CHARSET, DEFAULT_PITCH, DT_LEFT, DT_NOPREFIX, DT_TOP, DeleteObject, DrawTextW,
    EndPaint, FF_DONTCARE, FW_BOLD, FillRect, HGDIOBJ, OUT_DEFAULT_PRECIS, PAINTSTRUCT,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GWL_STYLE,
    GWLP_USERDATA, GetClientRect, GetWindowLongPtrW, GetWindowTextW, HTTRANSPARENT, HWND_TOP,
    LWA_COLORKEY, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_NCHITTEST, WM_PAINT,
    WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TRANSPARENT,
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
    visible: bool,
    font_size_pixels: i32,
}

impl D3d12HudOverlay {
    /// Create the transparent HUD child under an existing GUI-owned parent HWND.
    pub fn create(parent_handle: usize) -> Result<Self> {
        super::NativeVideoWindow::validate_existing_handle(parent_handle)?;
        let parent = HWND(parent_handle as *mut c_void);
        let instance = unsafe { GetModuleHandleW(None)? };
        register_hud_window_class(HINSTANCE(instance.0))?;

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_LAYERED.0 | WS_EX_TRANSPARENT.0 | WS_EX_NOACTIVATE.0),
                w!("SegaEmulatorTelemetryHud"),
                w!(""),
                WINDOW_STYLE(WS_CHILD.0 | WS_CLIPSIBLINGS.0),
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

        if let Err(error) =
            unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_COLORKEY) }
        {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            return Err(error);
        }
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 14);
        }

        let overlay = Self {
            hwnd,
            parent_hwnd: parent,
            _thread_affinity: PhantomData,
            visible: false,
            font_size_pixels: 14,
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
        let wide = safe_text
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
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
                SWP_NOACTIVATE,
            )?;
        }
        Ok(())
    }

    /// Set the GDI font height in physical pixels for the selected HUD size.
    pub fn set_font_size_pixels(&mut self, height_pixels: i32) -> Result<()> {
        if !(8..=48).contains(&height_pixels) {
            return Err(Error::new(
                windows::core::HRESULT(0x8007_0057_u32 as i32),
                "HUD font height must be between 8 and 48 physical pixels",
            ));
        }
        if self.font_size_pixels == height_pixels {
            return Ok(());
        }

        unsafe {
            SetWindowLongPtrW(
                self.hwnd,
                GWLP_USERDATA,
                isize::try_from(height_pixels).unwrap_or(14),
            );
            let mut text = [0_u16; 512];
            let _ = GetWindowTextW(self.hwnd, &mut text);
            SetWindowTextW(self.hwnd, PCWSTR(text.as_ptr()))?;
        }
        self.font_size_pixels = height_pixels;
        Ok(())
    }

    /// Show or hide the child HUD without changing focus or desktop z-order.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
        self.visible = visible;
    }

    /// Return whether this HWND is a direct child of the supplied parent.
    pub fn is_child_of(&self, parent_handle: usize) -> bool {
        let parent = HWND(parent_handle as *mut c_void);
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetParent(self.hwnd)
                .is_ok_and(|actual_parent| actual_parent == parent)
        }
    }

    /// Report whether the HWND has the required child/layered/pass-through styles.
    pub fn composition_contract(&self) -> (bool, bool, bool, bool, bool, bool) {
        let style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_STYLE) };
        let ex_style = unsafe { GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE) };
        (
            style & (WS_CHILD.0 as isize) != 0,
            ex_style & (WS_EX_LAYERED.0 as isize) != 0,
            ex_style & (WS_EX_TRANSPARENT.0 as isize) != 0,
            ex_style & (WS_EX_NOACTIVATE.0 as isize) != 0,
            style & (WS_CLIPSIBLINGS.0 as isize) != 0,
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
                    let font_height =
                        i32::try_from(unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) })
                            .unwrap_or(14)
                            .clamp(8, 48);
                    let font = unsafe {
                        CreateFontW(
                            -font_height,
                            0,
                            0,
                            0,
                            i32::try_from(FW_BOLD.0).unwrap_or(700),
                            0,
                            0,
                            0,
                            DEFAULT_CHARSET,
                            OUT_DEFAULT_PRECIS,
                            CLIP_DEFAULT_PRECIS,
                            ANTIALIASED_QUALITY,
                            u32::from(DEFAULT_PITCH.0) | u32::from(FF_DONTCARE.0),
                            w!("Segoe UI"),
                        )
                    };
                    let previous_font = if font.0.is_null() {
                        None
                    } else {
                        Some(unsafe { SelectObject(hdc, HGDIOBJ(font.0)) })
                    };
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
                    if let Some(previous_font) = previous_font {
                        unsafe {
                            SelectObject(hdc, previous_font);
                            let _ = DeleteObject(HGDIOBJ(font.0));
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

#[cfg(test)]
mod tests {
    use super::super::{D3d12ChildWindow, NativeVideoWindow};
    use super::D3d12HudOverlay;
    use super::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GW_CHILD, GetWindow};

    #[test]
    fn hud_is_a_transparent_non_topmost_child_above_the_d3d12_sibling() {
        let parent = NativeVideoWindow::create(320, 240).unwrap();
        let video = D3d12ChildWindow::create(parent.hwnd().0 as usize).unwrap();
        video.set_bounds(0, 0, 320, 240).unwrap();

        let mut hud = D3d12HudOverlay::create(parent.hwnd().0 as usize).unwrap();
        hud.set_bounds(8, 8, 160, 60).unwrap();
        hud.set_text("SMS 50.0 FPS\nFrame 20.00 ms\nInput -> Frame 8.0 ms")
            .unwrap();
        hud.set_font_size_pixels(22).unwrap();
        assert_eq!(
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                    hud.hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
                )
            },
            22
        );

        let mut client_rect = windows::Win32::Foundation::RECT::default();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetClientRect(hud.hwnd, &mut client_rect)
                .unwrap();
        }
        assert_eq!(client_rect.right - client_rect.left, 160);
        assert_eq!(client_rect.bottom - client_rect.top, 60);

        hud.set_bounds(12, 12, 240, 90).unwrap();
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetClientRect(hud.hwnd, &mut client_rect)
                .unwrap();
        }
        assert_eq!(client_rect.right - client_rect.left, 240);
        assert_eq!(client_rect.bottom - client_rect.top, 90);

        assert!(hud.is_child_of(parent.hwnd().0 as usize));
        assert_eq!(
            hud.composition_contract(),
            (true, true, true, true, true, true)
        );

        let parent_style = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                parent.hwnd(),
                windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
            )
        };
        assert_ne!(
            parent_style & (windows::Win32::UI::WindowsAndMessaging::WS_CLIPCHILDREN.0 as isize),
            0
        );
        let video_style = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                video.hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWL_STYLE,
            )
        };
        assert_ne!(
            video_style & (windows::Win32::UI::WindowsAndMessaging::WS_CLIPSIBLINGS.0 as isize),
            0
        );
        assert_eq!(
            unsafe {
                super::hud_window_proc(hud.hwnd, super::WM_MOUSEACTIVATE, WPARAM(0), LPARAM(0))
            },
            LRESULT(super::MA_NOACTIVATE as isize)
        );
        assert_eq!(
            unsafe { super::hud_window_proc(hud.hwnd, super::WM_NCHITTEST, WPARAM(0), LPARAM(0)) },
            LRESULT(super::HTTRANSPARENT as isize)
        );

        let first_child = unsafe { GetWindow(parent.hwnd(), GW_CHILD).unwrap() };
        assert_eq!(first_child, HWND(hud.handle() as *mut c_void));
    }
}
