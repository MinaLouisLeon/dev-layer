//! Discovering, depicting, and launching installed applications.
//!
//! Everything here is COM-flavoured Win32: resolving `.lnk` shortcuts through
//! `IShellLink`, pulling icons out of the shell image list, and handing launch
//! requests to the shell so shortcuts behave exactly as they do in the Start
//! Menu (working directory, arguments, elevation prompts and all).

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::{FILE_FLAGS_AND_ATTRIBUTES, WIN32_FIND_DATAW};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetFileInfoW, SHGetImageList, ShellExecuteW, ShellLink, SHFILEINFOW,
    SHGFI_SYSICONINDEX, SHIL_JUMBO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, GetIconInfo, HICON, ICONINFO, SW_SHOWNORMAL,
};

use crate::error::{Error, Result};

const MAX_PATH_CHARS: usize = 260;
/// Shell icons top out at 256×256 (SHIL_JUMBO); refuse anything larger as
/// obviously wrong rather than allocating from a bogus header.
const MAX_ICON_EDGE: i32 = 512;

/// COM apartment for the calling thread. Held for the duration of a catalog
/// scan; dropping it uninitializes.
pub struct ComGuard;

impl ComGuard {
    pub fn new() -> Result<Self> {
        // SAFETY: paired with CoUninitialize in Drop, on this same thread.
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr.is_err() {
            return Err(Error::Platform(format!("CoInitializeEx failed: {hr:?}")));
        }
        Ok(Self)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// What a `.lnk` file points at.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub target: PathBuf,
    pub arguments: String,
    pub working_dir: Option<PathBuf>,
}

/// Reads a shortcut without launching it. Requires a live [`ComGuard`].
pub fn resolve_shortcut(lnk: &Path) -> Result<Shortcut> {
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| Error::Platform(format!("CoCreateInstance(ShellLink): {e}")))?;
        let file: IPersistFile = link
            .cast()
            .map_err(|e| Error::Platform(format!("IPersistFile cast: {e}")))?;

        file.Load(PCWSTR(wide(lnk).as_ptr()), STGM_READ)
            .map_err(|e| Error::Platform(format!("load {}: {e}", lnk.display())))?;

        let mut buffer = [0u16; MAX_PATH_CHARS];
        let mut find_data = WIN32_FIND_DATAW::default();
        // SLGP_RAWPATH (4): keep environment variables unexpanded rather than
        // resolving against this process's environment.
        link.GetPath(&mut buffer, &mut find_data, 4)
            .map_err(|e| Error::Platform(format!("GetPath: {e}")))?;
        let target = from_wide(&buffer);
        if target.is_empty() {
            return Err(Error::Platform("shortcut has no target".into()));
        }

        let mut arg_buffer = [0u16; 1024];
        let arguments = link
            .GetArguments(&mut arg_buffer)
            .map(|_| from_wide(&arg_buffer))
            .unwrap_or_default();

        let mut dir_buffer = [0u16; MAX_PATH_CHARS];
        let working_dir = link
            .GetWorkingDirectory(&mut dir_buffer)
            .ok()
            .map(|_| from_wide(&dir_buffer))
            .filter(|d| !d.is_empty())
            .map(PathBuf::from);

        Ok(Shortcut {
            target: PathBuf::from(target),
            arguments,
            working_dir,
        })
    }
}

/// Renders a file's shell icon (the `.lnk`'s own icon, or an executable's) to
/// PNG bytes, cropped to its visible content.
pub fn extract_icon_png(path: &Path) -> Result<Vec<u8>> {
    let (rgba, width, height) = unsafe {
        let icon = system_icon(path)?;
        let result = icon_to_rgba(icon);
        let _ = DestroyIcon(icon);
        result?
    };

    let (rgba, width, height) = crop_transparent_border(rgba, width, height);
    encode_png(&rgba, width, height)
}

/// Hands the path to the shell, so shortcuts resolve exactly as they do from
/// the Start Menu — including UWP `shell:AppsFolder\…` targets.
pub fn launch(path: &Path, arguments: &str, working_dir: Option<&Path>) -> Result<()> {
    let file = wide(path);
    let args = wide_str(arguments);
    let dir = working_dir.map(wide);

    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(), // default verb: "open"
            PCWSTR(file.as_ptr()),
            if arguments.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(args.as_ptr())
            },
            dir.as_ref()
                .map(|d| PCWSTR(d.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            SW_SHOWNORMAL,
        )
    };

    // ShellExecuteW returns a fake HINSTANCE; values <= 32 are error codes.
    if result.0 as usize <= 32 {
        return Err(Error::Platform(format!(
            "launch failed for {} (code {})",
            path.display(),
            result.0 as usize
        )));
    }
    Ok(())
}

// ------------------------------------------------------------------ icons

/// The largest icon the shell has for this file (256×256 where available).
unsafe fn system_icon(path: &Path) -> Result<HICON> {
    let mut info = SHFILEINFOW::default();
    let found = unsafe {
        SHGetFileInfoW(
            PCWSTR(wide(path).as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX,
        )
    };
    if found == 0 {
        return Err(Error::Platform(format!(
            "no shell icon for {}",
            path.display()
        )));
    }

    let images: IImageList = unsafe { SHGetImageList(SHIL_JUMBO as i32) }
        .map_err(|e| Error::Platform(format!("SHGetImageList: {e}")))?;

    unsafe { images.GetIcon(info.iIcon, ILD_TRANSPARENT.0) }
        .map_err(|e| Error::Platform(format!("GetIcon: {e}")))
}

/// Converts an HICON to straight RGBA.
unsafe fn icon_to_rgba(icon: HICON) -> Result<(Vec<u8>, u32, u32)> {
    let mut icon_info = ICONINFO::default();
    unsafe { GetIconInfo(icon, &mut icon_info) }
        .map_err(|e| Error::Platform(format!("GetIconInfo: {e}")))?;

    let color = icon_info.hbmColor;
    let mask = icon_info.hbmMask;
    let cleanup = || unsafe {
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
    };

    let mut bitmap = BITMAP::default();
    let read = unsafe {
        GetObjectW(
            HGDIOBJ(color.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut std::ffi::c_void),
        )
    };
    if read == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        cleanup();
        return Err(Error::Platform("icon has no colour bitmap".into()));
    }
    if bitmap.bmWidth > MAX_ICON_EDGE || bitmap.bmHeight > MAX_ICON_EDGE {
        cleanup();
        return Err(Error::Platform("implausible icon dimensions".into()));
    }

    let width = bitmap.bmWidth as u32;
    let height = bitmap.bmHeight as u32;
    let mut header = BITMAPINFO::default();
    header.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    header.bmiHeader.biWidth = bitmap.bmWidth;
    // Negative height requests a top-down DIB, so row 0 is the top row.
    header.bmiHeader.biHeight = -bitmap.bmHeight;
    header.bmiHeader.biPlanes = 1;
    header.bmiHeader.biBitCount = 32;
    header.bmiHeader.biCompression = BI_RGB.0;

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let dc = unsafe { GetDC(None) };
    let copied = unsafe {
        GetDIBits(
            dc,
            color,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut std::ffi::c_void),
            &mut header,
            DIB_RGB_COLORS,
        )
    };
    unsafe { ReleaseDC(None, dc) };
    cleanup();

    if copied == 0 {
        return Err(Error::Platform("GetDIBits returned no scanlines".into()));
    }

    // GDI hands back BGRA; PNG wants RGBA.
    let mut opaque = false;
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        opaque |= pixel[3] != 0;
    }
    // Icons without an alpha channel come back fully transparent. They are
    // rare on modern Windows, but a silently invisible icon is worse than a
    // square one, so force them opaque.
    if !opaque {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }

    Ok((pixels, width, height))
}

/// Jumbo icons pad smaller art into a 256×256 canvas; trim it so the dock can
/// size icons itself.
fn crop_transparent_border(pixels: Vec<u8>, width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0u32, 0u32);

    for y in 0..height {
        for x in 0..width {
            if pixels[((y * width + x) * 4 + 3) as usize] > 8 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return (pixels, width, height); // fully transparent; leave it alone
    }

    let (new_width, new_height) = (max_x - min_x + 1, max_y - min_y + 1);
    if new_width == width && new_height == height {
        return (pixels, width, height);
    }

    let mut cropped = Vec::with_capacity((new_width * new_height * 4) as usize);
    for y in min_y..=max_y {
        let start = ((y * width + min_x) * 4) as usize;
        cropped.extend_from_slice(&pixels[start..start + (new_width * 4) as usize]);
    }
    (cropped, new_width, new_height)
}

fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|e| Error::Platform(format!("png header: {e}")))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| Error::Platform(format!("png data: {e}")))?;
    }
    Ok(out)
}

// ----------------------------------------------------------------- helpers

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn wide_str(value: &str) -> Vec<u16> {
    OsString::from(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn from_wide(buffer: &[u16]) -> String {
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    OsString::from_wide(&buffer[..end])
        .to_string_lossy()
        .trim()
        .to_string()
}
