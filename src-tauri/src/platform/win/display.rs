//! Monitor enumeration and display-change notifications.

use std::ffi::c_void;
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;

use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, EnumDisplayMonitors, GetMonitorInfoW, DISPLAY_DEVICEW, HDC, HMONITOR,
    MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
    PostMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
    GWLP_USERDATA, MONITORINFOF_PRIMARY, MSG, SPI_SETWORKAREA, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLOSE, WM_DESTROY, WM_DEVICECHANGE, WM_DISPLAYCHANGE, WM_DPICHANGED, WM_SETTINGCHANGE,
    WNDCLASSW, WS_EX_TOOLWINDOW,
};

use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::monitors::MonitorInfo;

const DEFAULT_DPI: u32 = 96;

// ---------------------------------------------------------------- monitors

pub fn enumerate_monitors() -> Result<Vec<MonitorInfo>> {
    let mut found: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut found as *mut Vec<MonitorInfo> as isize),
        );
    }

    if found.is_empty() {
        return Err(Error::Platform(
            "EnumDisplayMonitors returned no displays".into(),
        ));
    }
    Ok(found)
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let found = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    if let Some(info) = describe(monitor) {
        found.push(info);
    }
    TRUE // keep enumerating even if one monitor could not be described
}

unsafe fn describe(monitor: HMONITOR) -> Option<MonitorInfo> {
    let mut raw = MONITORINFOEXW::default();
    raw.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if !GetMonitorInfoW(monitor, &mut raw as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        tracing::warn!("GetMonitorInfoW failed for a monitor; skipping it");
        return None;
    }

    let device = wide_to_string(&raw.szDevice);
    if device.is_empty() {
        return None;
    }

    // Effective DPI is per-monitor: a 4K laptop panel next to 1080p externals
    // reports different values, and the HUD must scale per window.
    let mut dpi_x = DEFAULT_DPI;
    let mut dpi_y = DEFAULT_DPI;
    if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y).is_err() {
        dpi_x = DEFAULT_DPI;
    }

    Some(MonitorInfo {
        name: friendly_name(&device).unwrap_or_else(|| device.clone()),
        id: device,
        is_primary: raw.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        index: 0, // assigned by `monitors::normalize`
        bounds: rect_from(raw.monitorInfo.rcMonitor),
        work_area: rect_from(raw.monitorInfo.rcWork),
        scale_factor: dpi_x as f64 / DEFAULT_DPI as f64,
    })
}

/// Adapter-reported description, e.g. "Generic PnP Monitor". Real product names
/// need EDID parsing — worth doing once the dock shows per-monitor labels.
unsafe fn friendly_name(device: &str) -> Option<String> {
    let mut display = DISPLAY_DEVICEW {
        cb: std::mem::size_of::<DISPLAY_DEVICEW>() as u32,
        ..Default::default()
    };
    let wide: Vec<u16> = device.encode_utf16().chain(std::iter::once(0)).collect();

    if !EnumDisplayDevicesW(PCWSTR(wide.as_ptr()), 0, &mut display, 0).as_bool() {
        return None;
    }
    let name = wide_to_string(&display.DeviceString);
    (!name.is_empty()).then_some(name)
}

fn rect_from(r: RECT) -> Rect {
    Rect {
        x: r.left,
        y: r.top,
        width: r.right - r.left,
        height: r.bottom - r.top,
    }
}

fn wide_to_string(raw: &[u16]) -> String {
    let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    String::from_utf16_lossy(&raw[..end]).trim().to_string()
}

// ------------------------------------------------------- display notifier

/// A hidden top-level window whose only job is to receive display broadcasts.
///
/// It must be top-level: message-only (`HWND_MESSAGE`) windows are excluded
/// from broadcast messages such as `WM_DISPLAYCHANGE`.
pub struct DisplayNotifier {
    hwnd: isize,
    thread: Option<JoinHandle<()>>,
}

impl DisplayNotifier {
    pub fn stop(mut self) {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.hwnd as *mut c_void)),
                WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn spawn_display_notifier(tx: Sender<()>) -> Result<DisplayNotifier> {
    let (ready_tx, ready_rx) = mpsc::channel::<std::result::Result<isize, String>>();

    let thread = std::thread::Builder::new()
        .name("dev-layer/display-notifier".into())
        .spawn(move || unsafe {
            let hwnd = match create_notifier_window(tx) {
                Ok(hwnd) => {
                    let _ = ready_tx.send(Ok(hwnd.0 as isize));
                    hwnd
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                    return;
                }
            };

            // Dedicated message pump; this thread owns the window.
            let mut msg = MSG::default();
            // GetMessageW returns -1 on error, so test for > 0 rather than as_bool().
            while GetMessageW(&mut msg, Some(hwnd), 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        })
        .map_err(|e| Error::Platform(e.to_string()))?;

    match ready_rx.recv() {
        Ok(Ok(hwnd)) => Ok(DisplayNotifier {
            hwnd,
            thread: Some(thread),
        }),
        Ok(Err(e)) => Err(Error::Platform(format!("display notifier: {e}"))),
        Err(e) => Err(Error::Platform(format!(
            "display notifier never started: {e}"
        ))),
    }
}

unsafe fn create_notifier_window(tx: Sender<()>) -> Result<HWND> {
    let instance = GetModuleHandleW(None).map_err(|e| Error::Platform(e.to_string()))?;
    let class = w!("DevLayerDisplayNotifier");

    let wc = WNDCLASSW {
        lpfnWndProc: Some(notifier_proc),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // Non-zero on success; a duplicate registration is fine on restart.
    let _ = RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0),
        class,
        w!("dev-layer display notifier"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        None,
        None,
        Some(instance.into()),
        None,
    )
    .map_err(|e| Error::Platform(e.to_string()))?;

    // The window owns the sender; it is dropped in WM_DESTROY.
    let sender = Box::into_raw(Box::new(tx));
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, sender as isize);
    Ok(hwnd)
}

unsafe extern "system" fn notifier_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let notify = match msg {
        WM_DISPLAYCHANGE | WM_DPICHANGED | WM_DEVICECHANGE => true,
        // WM_SETTINGCHANGE is noisy; only the work-area change concerns us
        // (taskbar moved, resized, or auto-hide toggled).
        WM_SETTINGCHANGE => wparam.0 as u32 == SPI_SETWORKAREA.0,
        WM_DESTROY => {
            let sender = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Sender<()>;
            if !sender.is_null() {
                drop(Box::from_raw(sender));
            }
            PostQuitMessage(0);
            false
        }
        _ => false,
    };

    if notify {
        let sender = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Sender<()>;
        if !sender.is_null() {
            // A closed receiver just means we are shutting down.
            let _ = (*sender).send(());
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}
