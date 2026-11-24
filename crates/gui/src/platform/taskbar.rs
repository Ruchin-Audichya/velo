//! The count on Velo's taskbar button.
//!
//! How many chats are unread, drawn as a disc in the accent colour over the
//! button's corner — what the desktop messengers show, so a glance at the
//! taskbar answers "anything new?" without opening the window. Only Windows
//! has the overlay; elsewhere this is nothing.

/// What the badge says and how it looks. Colours come from the theme at the
/// call, since this module has no theme of its own.
#[derive(Clone, PartialEq, Debug)]
pub struct Badge {
    /// "1"–"99", or "99+" — the rail's own labels.
    pub label: String,
    /// What a screen reader says about the button.
    pub description: String,
    pub fill: gpui::Rgba,
    pub ink: gpui::Rgba,
}

/// The label for `count` unread chats, or nothing when there are none.
pub fn label(count: usize) -> Option<String> {
    match count {
        0 => None,
        1..=99 => Some(count.to_string()),
        _ => Some("99+".to_owned()),
    }
}

/// Show `badge` on the window's taskbar button, or clear it. Cheap to call
/// every frame: nothing reaches the shell unless the badge changed.
pub fn show(window: &gpui::Window, badge: Option<Badge>) {
    imp::show(window, badge);
}

#[cfg(not(windows))]
mod imp {
    pub(super) fn show(_: &gpui::Window, _: Option<super::Badge>) {}
}

#[cfg(windows)]
mod imp {
    use super::Badge;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::cell::RefCell;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS,
        CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET,
        DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC,
        DeleteObject, DrawTextW, FW_BOLD, GdiFlush, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode,
        SetTextColor, TRANSPARENT,
    };
    use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateIconIndirect, DestroyIcon, ICONINFO, RegisterWindowMessageW, SM_CXSMICON,
        WM_DPICHANGED, WM_NCDESTROY,
    };

    /// Not the placement subclass's id: both live on the same window.
    const SUBCLASS_ID: usize = 2;

    #[derive(Default)]
    struct State {
        hwnd: usize,
        /// What the button should show.
        wanted: Option<Badge>,
        /// Whether the shell has it. False after a failure, and after
        /// Explorer restarts and rebuilds every button without overlays.
        applied: bool,
        /// The pixel size the shell's copy was drawn at. A window dragged to
        /// a screen with another scale keeps the same badge but needs it
        /// redrawn: the size was part of what "unchanged" has to mean, and
        /// leaving it out kept a blurry or tiny badge until the count moved.
        applied_size: i32,
        subclassed: bool,
        taskbar: Option<windows::Win32::UI::Shell::ITaskbarList3>,
    }

    thread_local! {
        // The UI thread owns the window, the COM object and this.
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    pub(super) fn show(window: &gpui::Window, badge: Option<Badge>) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as usize;
        STATE.with_borrow_mut(|state| {
            if state.hwnd != hwnd {
                *state = State {
                    hwnd,
                    taskbar: state.taskbar.take(),
                    ..State::default()
                };
            }
            if !state.subclassed {
                // The shell announces a rebuilt button with this message;
                // the badge goes back on when it does.
                state.subclassed =
                    unsafe { SetWindowSubclass(hwnd as HWND, Some(on_message), SUBCLASS_ID, 0) }
                        != 0;
            }
            let size = icon_size(hwnd as HWND);
            if state.applied
                && state.wanted == badge
                && (badge.is_none() || state.applied_size == size)
            {
                return;
            }
            state.wanted = badge;
            apply(state);
        });
    }

    fn apply(state: &mut State) {
        let Some(taskbar) = taskbar(state) else {
            return;
        };
        let hwnd = windows::Win32::Foundation::HWND(state.hwnd as *mut _);
        let result = match &state.wanted {
            None => unsafe {
                taskbar.SetOverlayIcon(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::HICON::default(),
                    windows::core::PCWSTR::null(),
                )
            },
            Some(badge) => {
                let size = icon_size(state.hwnd as HWND);
                state.applied_size = size;
                let Some(icon) = draw(badge, size) else {
                    state.applied = false;
                    return;
                };
                let description = windows::core::HSTRING::from(badge.description.as_str());
                // The shell keeps its own copy; this one is ours to free.
                let result = unsafe {
                    taskbar.SetOverlayIcon(
                        hwnd,
                        windows::Win32::UI::WindowsAndMessaging::HICON(icon),
                        &description,
                    )
                };
                unsafe { DestroyIcon(icon) };
                result
            }
        };
        state.applied = result.is_ok();
    }

    fn taskbar(state: &mut State) -> Option<windows::Win32::UI::Shell::ITaskbarList3> {
        use windows::Win32::System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        };
        use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
        if state.taskbar.is_none() {
            // Already initialised on the UI thread in practice; a second call
            // on the same apartment is a no-op, a different one an error we
            // can ignore because the object still works there.
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            let taskbar: ITaskbarList3 =
                unsafe { CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER) }.ok()?;
            unsafe { taskbar.HrInit() }.ok()?;
            state.taskbar = Some(taskbar);
        }
        state.taskbar.clone()
    }

    /// The overlay's size in pixels: the small-icon size at the window's DPI,
    /// so the badge is drawn for the screen rather than scaled to it.
    fn icon_size(hwnd: HWND) -> i32 {
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        unsafe { GetSystemMetricsForDpi(SM_CXSMICON, dpi) }.clamp(16, 64)
    }

    /// The badge as an icon: an antialiased disc with the label knocked out
    /// of it in `ink`.
    fn draw(badge: &Badge, size: i32) -> Option<*mut core::ffi::c_void> {
        let coverage = text_coverage(&badge.label, size)?;
        let pixels = super::compose(&coverage, size as usize, badge.fill, badge.ink);
        unsafe {
            let (color, bits) = dib(size)?;
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits, pixels.len());
            // All zero: the mask lets the alpha channel decide everything.
            let mask_bits = vec![0u8; (size as usize).div_ceil(16) * 2 * size as usize];
            let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr().cast());
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&info);
            DeleteObject(mask);
            DeleteObject(color);
            (!icon.is_null()).then_some(icon)
        }
    }

    /// A top-down 32-bit bitmap and a pointer to its pixels.
    unsafe fn dib(size: i32) -> Option<(*mut core::ffi::c_void, *mut u8)> {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..unsafe { std::mem::zeroed() }
            },
            ..unsafe { std::mem::zeroed() }
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(
                std::ptr::null_mut(),
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            )
        };
        (!bitmap.is_null() && !bits.is_null()).then_some((bitmap, bits.cast()))
    }

    /// How much of each pixel the label covers, 0–255, drawn by GDI in
    /// grayscale antialiasing (never ClearType: its colour fringes would
    /// read as a smudge once used as a mask).
    fn text_coverage(label: &str, size: i32) -> Option<Vec<u8>> {
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return None;
            }
            let Some((bitmap, bits)) = dib(size) else {
                DeleteDC(dc);
                return None;
            };
            let old_bitmap = SelectObject(dc, bitmap);
            // Two digits fill the disc; "99+" needs a smaller face to fit
            // inside it: at half the disc its "+" was cut off at the edge.
            let height = if label.len() > 2 {
                size * 40 / 100
            } else {
                size * 68 / 100
            };
            let face: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
            let font = CreateFontW(
                -height,
                0,
                0,
                0,
                FW_BOLD as i32,
                0,
                0,
                0,
                u32::from(DEFAULT_CHARSET),
                u32::from(OUT_DEFAULT_PRECIS),
                u32::from(CLIP_DEFAULT_PRECIS),
                u32::from(ANTIALIASED_QUALITY),
                DEFAULT_PITCH as u32,
                face.as_ptr(),
            );
            let old_font = SelectObject(dc, font);
            SetTextColor(dc, 0x00FF_FFFF);
            SetBkMode(dc, TRANSPARENT as i32);
            let mut text: Vec<u16> = label.encode_utf16().collect();
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: size,
                bottom: size,
            };
            DrawTextW(
                dc,
                text.as_mut_ptr(),
                text.len() as i32,
                &mut rect,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            GdiFlush();
            let pixels = std::slice::from_raw_parts(bits, (size * size * 4) as usize);
            let coverage = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|px| px[0].max(px[1]).max(px[2]))
                .collect();
            SelectObject(dc, old_font);
            SelectObject(dc, old_bitmap);
            DeleteObject(font);
            DeleteObject(bitmap);
            DeleteDC(dc);
            Some(coverage)
        }
    }

    unsafe extern "system" fn on_message(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        subclass: usize,
        _: usize,
    ) -> LRESULT {
        if message == WM_NCDESTROY {
            unsafe { RemoveWindowSubclass(hwnd, Some(on_message), subclass) };
            STATE.with_borrow_mut(|state| {
                if state.hwnd == hwnd as usize {
                    state.subclassed = false;
                    state.applied = false;
                }
            });
        } else if message == WM_DPICHANGED {
            // Moved to a screen with another scale: draw the badge for it.
            let _ = STATE.try_with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && state.hwnd == hwnd as usize
                    && state.wanted.is_some()
                {
                    apply(&mut state);
                }
            });
        } else if message == button_created() {
            // `try_with`: a message can arrive while `show` holds the state,
            // and that call is about to apply the badge anyway.
            let _ = STATE.try_with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut()
                    && state.hwnd == hwnd as usize
                {
                    apply(&mut state);
                }
            });
        }
        unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
    }

    fn button_created() -> u32 {
        static MESSAGE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
        *MESSAGE.get_or_init(|| {
            let name: Vec<u16> = "TaskbarButtonCreated\0".encode_utf16().collect();
            unsafe { RegisterWindowMessageW(name.as_ptr()) }
        })
    }
}

/// The badge's pixels, as the 32-bit BGRA an icon wants (straight alpha):
/// a disc of `fill` filling the square, antialiased at its edge, with `ink`
/// wherever `coverage` says the label is.
#[cfg_attr(not(windows), allow(dead_code))]
fn compose(coverage: &[u8], size: usize, fill: gpui::Rgba, ink: gpui::Rgba) -> Vec<u8> {
    let centre = size as f32 / 2.0;
    let radius = centre - 0.25;
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut out = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - centre;
            let dy = y as f32 + 0.5 - centre;
            let disc = (radius - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
            let text = f32::from(coverage.get(y * size + x).copied().unwrap_or(0)) / 255.0;
            let mix = |a: f32, b: f32| a + (b - a) * text;
            out.extend_from_slice(&[
                channel(mix(fill.b, ink.b)),
                channel(mix(fill.g, ink.g)),
                channel(mix(fill.r, ink.r)),
                channel(disc * fill.a),
            ]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_matches_the_rail() {
        assert_eq!(label(0), None);
        assert_eq!(label(7).as_deref(), Some("7"));
        assert_eq!(label(99).as_deref(), Some("99"));
        assert_eq!(label(100).as_deref(), Some("99+"));
    }

    #[test]
    fn the_disc_is_round_and_the_label_takes_the_ink() {
        let fill = gpui::Rgba {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        };
        let ink = gpui::Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let size = 20;
        let mut coverage = vec![0u8; size * size];
        coverage[10 * size + 10] = 255;
        let pixels = compose(&coverage, size, fill, ink);
        let at = |x: usize, y: usize| &pixels[(y * size + x) * 4..(y * size + x) * 4 + 4];
        assert_eq!(at(0, 0)[3], 0, "the corner is outside the disc");
        assert_eq!(at(5, 10), [0, 255, 0, 255], "fill inside, in BGRA");
        assert_eq!(at(10, 10), [255, 255, 255, 255], "ink where the label is");
        let edge = at(0, 10)[3];
        assert!(edge > 0 && edge < 255, "the edge is antialiased: {edge}");
    }
}
