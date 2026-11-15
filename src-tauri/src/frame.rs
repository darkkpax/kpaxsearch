use crate::backdrop::Backdrop;
use std::{
    cell::{Cell, RefCell},
    sync::mpsc,
};

const SUBCLASS_ID: usize = 0x4b504158;
const WM_WINDOWPOSCHANGED: u32 = 0x0047;
const WM_NCDESTROY: u32 = 0x0082;
const WM_DISMISS_SEARCH: u32 = 0x8000 + 0x4b;
const WH_MOUSE_LL: i32 = 14;

thread_local! {
    static SEARCH_MOUSE_HOOK: Cell<Option<(isize, isize)>> = const { Cell::new(None) };
}

type Proc = unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize;
type MouseProc = unsafe extern "system" fn(i32, usize, isize) -> isize;
#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
struct Frame {
    radius: Cell<u32>,
    applying: Cell<bool>,
    size: Cell<(i32, i32, i32)>,
    backdrop: RefCell<Option<Backdrop>>,
    search: std::cell::OnceCell<tauri::WebviewWindow>,
}

#[link(name = "comctl32")]
extern "system" {
    fn IsWindowVisible(window: isize) -> i32;
    fn PostMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SetWindowsHookExW(kind: i32, proc: MouseProc, module: isize, thread: u32) -> isize;
    fn UnhookWindowsHookEx(hook: isize) -> i32;
    fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    fn SetWindowSubclass(window: isize, proc: Proc, id: usize, data: usize) -> i32;
    fn GetWindowSubclass(window: isize, proc: Proc, id: usize, data: *mut usize) -> i32;
    fn RemoveWindowSubclass(window: isize, proc: Proc, id: usize) -> i32;
    fn DefSubclassProc(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> isize;
}
#[link(name = "user32")]
extern "system" {
    fn GetWindowRect(window: isize, rect: *mut Rect) -> i32;
    fn GetDpiForWindow(window: isize) -> u32;
    fn SetWindowRgn(window: isize, region: isize, redraw: i32) -> i32;
    fn GetWindowLongPtrW(window: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(window: isize, index: i32, value: isize) -> isize;
    fn SetWindowPos(
        window: isize,
        after: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(window: isize, attribute: u32, value: *const i32, size: u32) -> i32;
}

fn diameter(radius: u32, dpi: u32, width: i32, height: i32) -> i32 {
    ((radius.min(40) as f64 * 2. * dpi as f64 / 96.).round() as i32)
        .min(width)
        .min(height)
        .max(0)
}

unsafe extern "system" fn outside_click(code: i32, message: usize, data: isize) -> isize {
    // MSLLHOOKSTRUCT starts with the screen position of the click.
    if code == 0 && matches!(message, 0x0201 | 0x0204 | 0x0207 | 0x020b) {
        SEARCH_MOUSE_HOOK.with(|state| {
            if let Some((window, _)) = state.get() {
                let point = *(data as *const Point);
                let mut bounds = Rect::default();
                if GetWindowRect(window, &mut bounds) != 0
                    && (!(bounds.left..bounds.right).contains(&point.x)
                        || !(bounds.top..bounds.bottom).contains(&point.y))
                {
                    PostMessageW(window, WM_DISMISS_SEARCH, 0, 0);
                }
            }
        });
    }
    CallNextHookEx(0, code, message, data)
}

unsafe fn watch_clicks(window: isize, visible: bool) {
    SEARCH_MOUSE_HOOK.with(|state| match (visible, state.get()) {
        (true, None) => {
            let hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                outside_click,
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            if hook != 0 {
                state.set(Some((window, hook)));
            }
        }
        (false, Some((_, hook))) => {
            state.set(None);
            UnhookWindowsHookEx(hook);
        }
        _ => {}
    });
}

unsafe fn clip(window: isize, frame: &Frame, force: bool) -> Result<(), String> {
    if frame.applying.get() {
        return Ok(());
    }
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetWindowRect(window, &mut rect) == 0 {
        return Err("не удалось определить размер окна".into());
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let rounding = diameter(frame.radius.get(), GetDpiForWindow(window), width, height);
    let key = (width, height, rounding);
    if !force && frame.size.get() == key {
        return Ok(());
    }
    frame.applying.set(true);
    frame.size.set(key);
    // Only composition clips the pixels: an HRGN edge is binary and visibly jagged.
    SetWindowRgn(window, 0, 1);
    if let Some(backdrop) = frame.backdrop.borrow().as_ref() {
        let _ = backdrop.resize(width, height, rounding);
    }
    frame.applying.set(false);
    Ok(())
}

unsafe extern "system" fn procedure(
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    data: usize,
) -> isize {
    // Keep the client area borderless even when Windows changes theme, activation or styles.
    match message {
        0x0083 | 0x0085 => return 0,
        0x0086 => return 1,
        WM_DISMISS_SEARCH => {
            if let Some(search) = (*(data as *const Frame)).search.get() {
                let search = search.clone();
                let _ = search.hide();
            }
            return 0;
        }
        _ => {}
    }
    let result = DefSubclassProc(window, message, wparam, lparam);
    if message == WM_NCDESTROY {
        if (*(data as *const Frame)).search.get().is_some() {
            watch_clicks(window, false);
        }
        RemoveWindowSubclass(window, procedure, SUBCLASS_ID);
        drop(Box::from_raw(data as *mut Frame));
    } else if matches!(
        message,
        0x0005 | WM_WINDOWPOSCHANGED | 0x007d | 0x02e0 | 0x031a
    ) {
        let frame = &*(data as *const Frame);
        if message == WM_WINDOWPOSCHANGED && frame.search.get().is_some() {
            watch_clicks(window, IsWindowVisible(window) != 0);
        }
        let _ = clip(window, frame, message != 0x0005);
    }
    result
}

unsafe fn ensure(window: isize) -> Result<&'static Frame, String> {
    let mut data = 0;
    if GetWindowSubclass(window, procedure, SUBCLASS_ID, &mut data) == 0 {
        data = Box::into_raw(Box::new(Frame {
            radius: Cell::new(16),
            applying: Cell::new(false),
            size: Cell::new((0, 0, 0)),
            backdrop: RefCell::new(None),
            search: std::cell::OnceCell::new(),
        })) as usize;
        if SetWindowSubclass(window, procedure, SUBCLASS_ID, data) == 0 {
            drop(Box::from_raw(data as *mut Frame));
            return Err("не удалось настроить рамку окна".into());
        }
        DwmSetWindowAttribute(window, 33, &1, 4);
        DwmSetWindowAttribute(window, 34, &-2, 4);
    }
    Ok(&*(data as *const Frame))
}

pub fn dismiss_search_on_outside_click(window: &tauri::WebviewWindow) -> Result<(), String> {
    let handle = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    // Setup runs on the UI thread, which receives the mouse hook callbacks.
    unsafe {
        let _ = ensure(handle)?.search.set(window.clone());
        watch_clicks(handle, IsWindowVisible(handle) != 0);
    }
    Ok(())
}

fn on_ui(
    window: tauri::Window,
    action: impl FnOnce(isize) -> Result<(), String> + Send + 'static,
) -> Result<(), String> {
    let handle = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let (tx, rx) = mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = tx.send(action(handle));
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn set_rounding(window: tauri::Window, radius: u32) -> Result<(), String> {
    on_ui(window, move |handle| unsafe {
        let frame = ensure(handle)?;
        frame.radius.set(radius.min(40));
        clip(handle, frame, true)
    })
}

#[tauri::command]
pub fn set_glass(
    window: tauri::Window,
    material: String,
    dark: bool,
    density: u32,
) -> Result<(), String> {
    on_ui(window, move |handle| unsafe {
        let frame = ensure(handle)?;
        if !matches!(material.as_str(), "none" | "blur" | "acrylic" | "frosted") {
            return Err("неизвестный материал".into());
        }
        DwmSetWindowAttribute(handle, 38, &1, 4);
        DwmSetWindowAttribute(handle, 17, &1, 4);
        let style = GetWindowLongPtrW(handle, -16);
        let borderless = (style & !0x00cf0000isize) | 0x80000000u32 as isize;
        if style != borderless {
            SetWindowLongPtrW(handle, -16, borderless);
            SetWindowPos(handle, 0, 0, 0, 0, 0, 0x37);
        }
        let extended = GetWindowLongPtrW(handle, -20);
        if extended & 0x00200000 == 0 {
            SetWindowLongPtrW(handle, -20, extended | 0x00200000);
            SetWindowPos(handle, 0, 0, 0, 0, 0, 0x37);
        }
        if frame.backdrop.borrow().is_none() {
            frame.backdrop.replace(Some(
                Backdrop::new(handle).map_err(|e| format!("не удалось создать стекло: {e}"))?,
            ));
        }
        frame
            .backdrop
            .borrow()
            .as_ref()
            .unwrap()
            .appearance(&material, dark, density)
            .map_err(|e| e.to_string())?;
        clip(handle, frame, true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn radius_scales_with_display_dpi() {
        assert_eq!(diameter(16, 144, 620, 372), 48);
    }
    #[test]
    fn short_search_window_caps_corner_diameter() {
        assert_eq!(diameter(40, 96, 620, 56), 56);
    }
}
