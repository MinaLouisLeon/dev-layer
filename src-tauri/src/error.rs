use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// A Win32/platform call failed.
    Platform(String),
    /// Config could not be read or written; callers fall back to defaults.
    Config(String),
    /// A HUD window could not be created, placed, or resolved.
    Hud(String),
    Tauri(tauri::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Platform(m) => write!(f, "platform: {m}"),
            Error::Config(m) => write!(f, "config: {m}"),
            Error::Hud(m) => write!(f, "hud: {m}"),
            Error::Tauri(e) => write!(f, "tauri: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<tauri::Error> for Error {
    fn from(e: tauri::Error) -> Self {
        Error::Tauri(e)
    }
}

/// IPC boundary: commands return plain strings so the frontend gets readable errors.
impl From<Error> for String {
    fn from(e: Error) -> String {
        e.to_string()
    }
}
