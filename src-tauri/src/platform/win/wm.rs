//! Window management primitives: which windows exist, which of them are ours
//! to arrange, where they are, and how to move them.
//!
//! Nothing here decides *layout* — that is `wm::layout`, which is pure and
//! tested. This module is the Win32 half only.

use std::ffi::c_void;
use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::thread::JoinHandle;

use parking_lot::Mutex;
use windows::core::BOOL;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, EnumWindows, GetAncestor, GetForegroundWindow, GetMessageW, GetWindow,
    GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
    IsWindowVisible, IsZoomed, PostMessageW, PostThreadMessageW, SetForegroundWindow, SetWindowPos,
    ShowWindow, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND,
    EVENT_SYSTEM_MINIMIZEEND, EVENT_SYSTEM_MINIMIZESTART, GA_ROOT, GWL_EXSTYLE, GWL_STYLE,
    GW_OWNER, MSG, OBJID_WINDOW, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOZORDER, SW_RESTORE,
    SW_SHOWNOACTIVATE, WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS, WM_CLOSE, WM_QUIT, WS_CHILD,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::error::{Error, Result};
use crate::geometry::Rect;

/// Windows smaller than this are splash screens, tooltips and IME popups.
const MIN_MANAGEABLE_EDGE: i32 = 120;

/// Shell surfaces that are technically top-level windows but are the desktop.
const CLASS_BLOCKLIST: &[&str] = &[
    "Progman",
    "WorkerW",
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
    "ForegroundStaging",
    "MultitaskingViewFrame",
    "TaskListThumbnailWnd",
];

#[derive(Debug, Clone)]
pub struct NativeWindow {
    /// The `HWND` as an integer. Not stable across restarts of the *app*, but
    /// stable while the window lives, which is all the WM needs.
    pub hwnd: isize,
    pub title: String,
    pub class: String,
    pub pid: u32,
    /// Executable file name, e.g. `Code.exe`. Used for per-app rules.
    pub process: String,
    pub rect: Rect,
    pub minimized: bool,
    pub maximized: bool,
}

/// What the watcher reports. Deliberately coarse: the WM re-enumerates on a
/// debounce rather than trying to track individual windows from events.
#[derive(Debug, Clone, Copy)]
pub enum WindowEvent {
    Changed,
    Foreground(isize),
}

// ------------------------------------------------------------ enumeration

pub fn enumerate_windows() -> Vec<NativeWindow> {
    let mut found: Vec<NativeWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut found as *mut Vec<NativeWindow> as isize),
        );
    }
    found
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let found = unsafe { &mut *(lparam.0 as *mut Vec<NativeWindow>) };
    if let Some(window) = describe(hwnd) {
        found.push(window);
    }
    TRUE
}

/// Describes a window if it is one we could tile; `None` otherwise.
pub fn describe(hwnd: HWND) -> Option<NativeWindow> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        // Top-level only: child and owned windows (dialogs, popups) follow
        // their owner rather than taking a tile of their own.
        if GetAncestor(hwnd, GA_ROOT) != hwnd
            || !GetWindow(hwnd, GW_OWNER).unwrap_or_default().is_invalid()
        {
            return None;
        }

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if style & WS_CHILD.0 != 0
            || ex_style & WS_EX_TOOLWINDOW.0 != 0
            || ex_style & WS_EX_NOACTIVATE.0 != 0
        {
            return None;
        }

        // Cloaked windows are the ghosts of UWP apps and other virtual
        // desktops: visible to EnumWindows, invisible to the user.
        let mut cloaked = 0u32;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
        {
            return None;
        }

        let title = text(|buffer| GetWindowTextW(hwnd, buffer));
        if title.is_empty() {
            return None;
        }
        let class =
            text(|buffer| windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, buffer));
        if CLASS_BLOCKLIST.contains(&class.as_str()) {
            return None;
        }

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        // Never manage our own HUD windows.
        if pid == GetCurrentProcessId() {
            return None;
        }

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).ok()?;
        let rect = Rect {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        };
        let minimized = IsIconic(hwnd).as_bool();
        if !minimized && (rect.width < MIN_MANAGEABLE_EDGE || rect.height < MIN_MANAGEABLE_EDGE) {
            return None;
        }

        Some(NativeWindow {
            hwnd: hwnd.0 as isize,
            title,
            class,
            process: process_name(pid).unwrap_or_default(),
            pid,
            rect,
            minimized,
            maximized: IsZoomed(hwnd).as_bool(),
        })
    }
}

pub fn window_info(hwnd: isize) -> Option<NativeWindow> {
    describe(handle(hwnd))
}

pub fn foreground_window() -> Option<isize> {
    let hwnd = unsafe { GetForegroundWindow() };
    (!hwnd.is_invalid()).then(|| hwnd.0 as isize)
}

// ------------------------------------------------------------ manipulation

/// Moves and resizes without raising or focusing: tiling must never steal the
/// window you are typing into.
pub fn set_window_rect(hwnd: isize, rect: Rect) -> Result<()> {
    unsafe {
        SetWindowPos(
            handle(hwnd),
            None,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOCOPYBITS,
        )
        .map_err(|e| Error::Platform(format!("SetWindowPos: {e}")))
    }
}

/// Un-minimizes and un-maximizes so a window can be tiled. Maximized windows
/// ignore `SetWindowPos` geometry, which looks like the WM silently failing.
pub fn restore_window(hwnd: isize) {
    unsafe {
        let handle = handle(hwnd);
        if IsIconic(handle).as_bool() {
            let _ = ShowWindow(handle, SW_RESTORE);
        } else if IsZoomed(handle).as_bool() {
            // Restore without activating, so a background window being untiled
            // does not jump to the front.
            let _ = ShowWindow(handle, SW_SHOWNOACTIVATE);
        }
    }
}

pub fn focus_window(hwnd: isize) -> Result<()> {
    unsafe {
        let handle = handle(hwnd);
        if IsIconic(handle).as_bool() {
            let _ = ShowWindow(handle, SW_RESTORE);
        }
        if !SetForegroundWindow(handle).as_bool() {
            // Windows refuses foreground changes from background processes in
            // some states; not fatal, and not worth fighting with tricks.
            return Err(Error::Platform("focus refused by the shell".into()));
        }
        Ok(())
    }
}

pub fn close_window(hwnd: isize) -> Result<()> {
    unsafe {
        PostMessageW(
            Some(handle(hwnd)),
            WM_CLOSE,
            Default::default(),
            Default::default(),
        )
        .map_err(|e| Error::Platform(format!("WM_CLOSE: {e}")))
    }
}

/// Current `(style, ex_style)`, so the WM can put a window back exactly as it
/// found it.
pub fn window_styles(hwnd: isize) -> (u32, u32) {
    unsafe {
        let handle = handle(hwnd);
        (
            GetWindowLongPtrW(handle, GWL_STYLE) as u32,
            GetWindowLongPtrW(handle, GWL_EXSTYLE) as u32,
        )
    }
}

// --------------------------------------------------------------- watcher

static EVENT_SENDER: OnceLock<Mutex<Option<Sender<WindowEvent>>>> = OnceLock::new();

fn sender_slot() -> &'static Mutex<Option<Sender<WindowEvent>>> {
    EVENT_SENDER.get_or_init(|| Mutex::new(None))
}

pub struct WindowWatcher {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl WindowWatcher {
    pub fn stop(mut self) {
        unsafe {
            let _ = PostThreadMessageW(
                self.thread_id,
                WM_QUIT,
                Default::default(),
                Default::default(),
            );
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        *sender_slot().lock() = None;
    }
}

/// Hooks the shell's window events. The hook must live on a thread with a
/// message pump, and `WINEVENT_OUTOFCONTEXT` means our callback runs on that
/// thread rather than inside every other process.
pub fn spawn_window_watcher(tx: Sender<WindowEvent>) -> Result<WindowWatcher> {
    *sender_slot().lock() = Some(tx);

    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<u32>();
    let thread = std::thread::Builder::new()
        .name("dev-layer/window-watcher".into())
        .spawn(move || unsafe {
            let events = [
                (EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY),
                (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
                (EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE),
                (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                (EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND),
            ];
            let hooks: Vec<HWINEVENTHOOK> = events
                .iter()
                .map(|(first, last)| {
                    SetWinEventHook(
                        *first,
                        *last,
                        None,
                        Some(window_event_proc),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                    )
                })
                .filter(|hook| !hook.is_invalid())
                .collect();

            let _ = ready_tx.send(windows::Win32::System::Threading::GetCurrentThreadId());

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                DispatchMessageW(&msg);
            }

            for hook in hooks {
                let _ = UnhookWinEvent(hook);
            }
        })
        .map_err(|e| Error::Platform(e.to_string()))?;

    let thread_id = ready_rx
        .recv()
        .map_err(|e| Error::Platform(format!("window watcher never started: {e}")))?;

    Ok(WindowWatcher {
        thread_id,
        thread: Some(thread),
    })
}

unsafe extern "system" fn window_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    // Only whole windows. Without this, every caret blink and menu highlight
    // in every application arrives here.
    if id_object != OBJID_WINDOW.0 || id_child != 0 || hwnd.is_invalid() {
        return;
    }

    let guard = sender_slot().lock();
    let Some(sender) = guard.as_ref() else {
        return;
    };

    let message = if event == EVENT_SYSTEM_FOREGROUND {
        WindowEvent::Foreground(hwnd.0 as isize)
    } else {
        WindowEvent::Changed
    };
    let _ = sender.send(message);
}

// --------------------------------------------------------------- helpers

fn handle(hwnd: isize) -> HWND {
    HWND(hwnd as *mut c_void)
}

/// Calls a Win32 text getter with a buffer and trims it to the returned length.
fn text(mut get: impl FnMut(&mut [u16]) -> i32) -> String {
    let mut buffer = [0u16; 512];
    let length = get(&mut buffer);
    if length <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..length as usize])
        .trim()
        .to_string()
}

fn process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let mut buffer = [0u16; 512];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(handle);
        result.ok()?;

        let full = String::from_utf16_lossy(&buffer[..length as usize]);
        Some(full.rsplit('\\').next().unwrap_or(&full).to_string())
    }
}
