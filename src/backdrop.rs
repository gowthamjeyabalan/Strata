//! The window's backdrop material, as in Windows 11 apps: Mica (tinted by the wallpaper), Mica Alt,
//! or Acrylic (a blur of what is behind the window). Windows draws it behind the window; the panels
//! are see-through so it shows. Needs Windows 11 22H2 or later; elsewhere nothing happens.

use crate::theme::Backdrop;

/// The native window, found once at start.
#[derive(Clone, Copy, Default)]
pub struct Window(isize);

impl Window {
    pub fn of(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match cc.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => Window(h.hwnd.get()),
            _ => Window(0),
        }
    }

    /// Ask Windows for `backdrop` behind the window, with dark or light tinting (which also colours
    /// the title bar). Returns false when Windows cannot do it.
    #[cfg(windows)]
    pub fn set(self, backdrop: Backdrop, dark: bool) -> bool {
        use windows_sys::Win32::Graphics::Dwm::{DwmExtendFrameIntoClientArea, DwmSetWindowAttribute};
        use windows_sys::Win32::UI::Controls::MARGINS;
        const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
        const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
        if self.0 == 0 {
            return false;
        }
        let hwnd = self.0 as *mut core::ffi::c_void;
        let kind: i32 = match backdrop {
            Backdrop::None => 1,
            Backdrop::Mica => 2,
            Backdrop::Acrylic => 3,
            Backdrop::MicaAlt => 4,
        };
        let dark = dark as i32;
        // Safety: the window lives as long as the app, and each value is the size Windows expects.
        unsafe {
            DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const i32 as *const _, 4);
            // The material reaches into the whole window, not only the title bar.
            let m = if backdrop == Backdrop::None { 0 } else { -1 };
            DwmExtendFrameIntoClientArea(hwnd, &MARGINS { cxLeftWidth: m, cxRightWidth: m, cyTopHeight: m, cyBottomHeight: m });
            DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &kind as *const i32 as *const _, 4) == 0
        }
    }

    #[cfg(not(windows))]
    pub fn set(self, _backdrop: Backdrop, _dark: bool) -> bool {
        false
    }
}
