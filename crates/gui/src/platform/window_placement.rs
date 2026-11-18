//! Keep the window — and above all its title bar — inside the usable screen.
//!
//! Two ways a title bar ended up above the top of the screen, and so out of
//! reach of the mouse that could have moved it back:
//!
//! - **Opening.** The window is sized for its *content*; Windows then adds the
//!   caption and borders. On a laptop whose work area is barely taller than
//!   the content (1920×1080 at 125% leaves 816 logical pixels above the
//!   taskbar) the finished window is taller than the space, and centring it
//!   puts its top above the screen. Fixed by fitting the window's real, visible
//!   frame into the monitor's work area once it exists, shrinking it if it has
//!   to.
//! - **Dragging.** Win32 lets a caption be dragged above the work area. Fixed
//!   by adjusting the rectangle in `WM_MOVING`, so the drag stops at the top.
//!
//! The same fit runs again after a move or resize ends and when the display
//! or taskbar changes, because each of those can leave the window hanging
//! off an edge. A maximised window is left to Windows, which already fits it.

pub(crate) fn keep_title_bar_on_screen(window: &gpui::Window) {
    imp::install(window);
}

/// A rectangle in screen pixels: left, top, right, bottom.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg_attr(not(any(windows, test)), allow(dead_code))]
impl Rect {
    fn width(self) -> i32 {
        self.right - self.left
    }
    fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// Where `frame` has to go to lie inside `work`, or `None` when it already
/// does.
///
/// Shrinks a frame that is larger than the work area, then slides it in from
/// whichever edge it overhangs. The top wins over the bottom: a window that
/// still cannot fit keeps its title bar on screen, because the title bar is
/// how it gets moved.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn fit_into(frame: Rect, work: Rect) -> Option<Rect> {
    if work.width() <= 0 || work.height() <= 0 {
        return None;
    }
    let width = frame.width().min(work.width());
    let height = frame.height().min(work.height());
    let left = frame.left.min(work.right - width).max(work.left);
    let top = frame.top.min(work.bottom - height).max(work.top);
    let fitted = Rect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    };
    (fitted != frame).then_some(fitted)
}

#[cfg(windows)]
mod imp {
    use super::{Rect, fit_into};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromRect,
        MonitorFromWindow, OffsetRect,
    };
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetWindowRect, IsIconic, IsZoomed, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
        SWP_NOZORDER, SetWindowPos, WM_DISPLAYCHANGE, WM_EXITSIZEMOVE, WM_MOVING, WM_NCDESTROY,
        WM_SETTINGCHANGE, WM_SHOWWINDOW,
    };

    const SUBCLASS_ID: usize = 1;

    pub(super) fn install(window: &gpui::Window) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as HWND;
        // The UI thread owns both the window and this callback. No Rust state
        // is retained by Win32.
        let installed = unsafe { SetWindowSubclass(hwnd, Some(handle_message), SUBCLASS_ID, 0) };
        if installed == 0 {
            log::warn!("Could not enable title bar placement protection");
        }
        // The window exists at its opening size and position now; correct it
        // before the first frame is shown rather than after.
        fit_on_screen(hwnd);
    }

    unsafe extern "system" fn handle_message(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        subclass: usize,
        _: usize,
    ) -> LRESULT {
        match message {
            WM_NCDESTROY => {
                // The HWND is still valid here, and removal precedes destruction.
                unsafe { RemoveWindowSubclass(hwnd, Some(handle_message), subclass) };
            }
            WM_MOVING if lparam != 0 => {
                // WM_MOVING lends a writable screen-coordinate RECT for this call.
                let moving = unsafe { &mut *(lparam as *mut RECT) };
                if let Some(top) = cursor_work_top(moving) {
                    let offset = top.saturating_sub(moving.top);
                    if offset > 0 {
                        // Move both edges together so dragging never resizes.
                        unsafe { OffsetRect(moving, 0, offset) };
                        return 1;
                    }
                }
            }
            WM_SHOWWINDOW if wparam != 0 => {
                let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
                fit_on_screen(hwnd);
                return result;
            }
            WM_EXITSIZEMOVE | WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
                let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
                fit_on_screen(hwnd);
                return result;
            }
            _ => {}
        }
        // The normal procedure handles resize, maximize, system menus and snap.
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    fn rect(r: RECT) -> Rect {
        Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }

    /// Fit the window's visible frame into its monitor's work area.
    ///
    /// The visible frame, not `GetWindowRect`: on Windows 10 and 11 the
    /// window rectangle includes invisible resize borders several pixels
    /// wide, and fitting those would leave a visible gap at the screen edge.
    /// The difference between the two is carried back onto the rectangle
    /// handed to `SetWindowPos`, which wants the outer one.
    fn fit_on_screen(hwnd: HWND) {
        if unsafe { IsZoomed(hwnd) != 0 || IsIconic(hwnd) != 0 } {
            return;
        }
        let mut outer = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut outer) } == 0 {
            return;
        }
        let mut frame = outer;
        let read = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS as u32,
                (&raw mut frame).cast(),
                std::mem::size_of::<RECT>() as u32,
            )
        };
        if read < 0 || frame.right <= frame.left {
            frame = outer;
        }
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let display = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
        if unsafe { GetMonitorInfoW(display, &mut monitor) } == 0 {
            return;
        }
        let Some(fitted) = fit_into(rect(frame), rect(monitor.rcWork)) else {
            return;
        };
        // Invisible border on each side, added back for SetWindowPos.
        let (left, top) = (frame.left - outer.left, frame.top - outer.top);
        let (right, bottom) = (outer.right - frame.right, outer.bottom - frame.bottom);
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                fitted.left - left,
                fitted.top - top,
                fitted.width() + left + right,
                fitted.height() + top + bottom,
                SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE,
            )
        };
    }

    fn cursor_work_top(moving: &RECT) -> Option<i32> {
        let mut cursor = POINT::default();
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // Use the pointer's monitor while crossing displays. Win32 coordinates
        // can be negative when a screen is arranged above the primary screen.
        let success = unsafe {
            let display = if GetCursorPos(&mut cursor) != 0 {
                MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
            } else {
                MonitorFromRect(moving, MONITOR_DEFAULTTONEAREST)
            };
            GetMonitorInfoW(display, &mut monitor) != 0
        };
        success.then_some(monitor.rcWork.top)
    }
}

#[cfg(not(windows))]
mod imp {
    pub(super) fn install(_: &gpui::Window) {}
}

#[cfg(test)]
mod tests {
    use super::{Rect, fit_into};

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The window as it actually opened on a 1920×1080 laptop at 125%:
    /// taller than the work area above the taskbar, with its top cut off.
    #[test]
    fn a_window_taller_than_the_work_area_is_shrunk_and_its_title_bar_shown() {
        let work = r(0, 0, 1920, 1020);
        let frame = r(73, -29, 1826, 1038);
        let fitted = fit_into(frame, work).expect("needs moving");
        assert_eq!(fitted.top, 0, "the title bar is on screen");
        assert!(
            fitted.bottom <= work.bottom,
            "and the bottom above the taskbar"
        );
        assert_eq!(fitted.height(), work.height());
        assert_eq!(fitted.width(), frame.width(), "width was fine and is kept");
    }

    #[test]
    fn a_window_that_fits_is_left_alone() {
        assert_eq!(fit_into(r(100, 100, 900, 700), r(0, 0, 1920, 1040)), None);
    }

    #[test]
    fn a_window_hanging_off_an_edge_slides_back_without_resizing() {
        let fitted = fit_into(r(-200, 900, 600, 1400), r(0, 0, 1920, 1040)).unwrap();
        assert_eq!(fitted, r(0, 540, 800, 1040));
    }

    #[test]
    fn a_second_monitor_above_the_primary_uses_its_own_coordinates() {
        let fitted = fit_into(r(0, -1200, 800, -500), r(0, -1080, 1920, -40)).unwrap();
        assert_eq!(fitted.top, -1080);
        assert_eq!(fitted.height(), 700);
    }
}
