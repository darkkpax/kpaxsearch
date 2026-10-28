use std::{cell::Cell, ffi::c_void, sync::mpsc};

const SUBCLASS_ID: usize = 0x4b504158;
type Proc = unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize;
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Accent {
    state: u32,
    flags: u32,
    color: u32,
    animation: u32,
}
#[repr(C)]
struct Composition {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}
struct Frame {
    radius: Cell<u32>,
    applying: Cell<bool>,
    size: Cell<(i32, i32, i32)>,
}

#[link(name = "comctl32")]
extern "system" {
    fn SetWindowSubclass(window: isize, proc: Proc, id: usize, data: usize) -> i32;
    fn GetWindowSubclass(window: isize, proc: Proc, id: usize, data: *mut usize) -> i32;
    fn RemoveWindowSubclass(window: isize, proc: Proc, id: usize) -> i32;
    fn DefSubclassProc(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
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
#[link(name = "gdi32")]
extern "system" {
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> isize;
    fn DeleteObject(object: isize) -> i32;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(window: isize, attribute: u32, value: *const i32, size: u32) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(name: *const u8) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *mut c_void;
}

fn diameter(radius: u32, dpi: u32, width: i32, height: i32) -> i32 {
    ((radius.min(40) as f64 * 2. * dpi as f64 / 96.).round() as i32)
        .min(width)
        .min(height)
        .max(0)
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
    let region = CreateRoundRectRgn(0, 0, width, height, rounding, rounding);
    if region == 0 {
        return Err("не удалось создать форму окна".into());
    }
    frame.applying.set(true);
    frame.size.set(key);
    let result = SetWindowRgn(window, region, 1);
    frame.applying.set(false);
    if result == 0 {
        DeleteObject(region);
        return Err("не удалось применить форму окна".into());
    }
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
        _ => {}
    }
    let result = DefSubclassProc(window, message, wparam, lparam);
    if message == 0x0082 {
        RemoveWindowSubclass(window, procedure, SUBCLASS_ID);
        drop(Box::from_raw(data as *mut Frame));
    } else if matches!(message, 0x0005 | 0x0047 | 0x007d | 0x02e0 | 0x031a) {
        let _ = clip(window, &*(data as *const Frame), message != 0x0005);
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
        let state = match material.as_str() {
            "none" => 0,
            "blur" => 3,
            "acrylic" | "frosted" => 4,
            _ => return Err("неизвестный материал".into()),
        };
        DwmSetWindowAttribute(handle, 38, &1, 4);
        let style = GetWindowLongPtrW(handle, -16);
        let borderless = (style & !0x00cf0000isize) | 0x80000000u32 as isize;
        if style != borderless {
            SetWindowLongPtrW(handle, -16, borderless);
            SetWindowPos(handle, 0, 0, 0, 0, 0, 0x37);
        }
        let opacity = match material.as_str() {
            "blur" => 30 + density.min(100) * 130 / 100,
            "frosted" => 170 + density.min(100) * 70 / 100,
            _ => 75 + density.min(100) * 140 / 100,
        };
        let tint = if dark { 0x001c1918 } else { 0x00f8fafb };
        let mut accent = Accent {
            state,
            flags: if state == 3 { 2 } else { 0 },
            color: (opacity << 24) | tint,
            animation: 0,
        };
        let mut data = Composition {
            attribute: 19,
            data: &mut accent as *mut _ as *mut c_void,
            size: std::mem::size_of::<Accent>(),
        };
        let address = GetProcAddress(
            GetModuleHandleA(b"user32.dll\0".as_ptr()),
            b"SetWindowCompositionAttribute\0".as_ptr(),
        );
        if address.is_null() {
            return Err("системное размытие недоступно".into());
        }
        let apply: unsafe extern "system" fn(isize, *mut Composition) -> i32 =
            std::mem::transmute(address);
        if apply(handle, &mut data) == 0 {
            return Err("windows не применила размытие".into());
        }
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
